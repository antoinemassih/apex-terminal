//! Tauri commands for XOL Export / Import.
//!
//! The FE shows a file picker, then either passes the chosen target path
//! (export) or the file's bytes (import) to these commands. We do the heavy
//! lifting against the live PostgreSQL pool.

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::codec::{db, xol};
use super::file_io;

/// Result of an import — the new chart's UUID plus any non-blocking warnings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub chart_id: String,
    pub warnings: xol::ImportWarnings,
}

/// Export a chart by UUID as XOL bytes. Returns the raw zip — FE picks where
/// to save it and writes via the file dialog.
#[tauri::command]
pub async fn export_chart_xol(chart_id: String) -> Result<Vec<u8>, String> {
    let pool = crate::drawing_db::get_pool()
        .ok_or_else(|| "DB not initialized".to_string())?;
    let id = Uuid::parse_str(&chart_id).map_err(|e| format!("invalid uuid: {e}"))?;
    let state = db::load_chart(pool, id).await.map_err(|e| e.to_string())?;
    xol::write(&state).map_err(|e| e.to_string())
}

/// Import an XOL file into the database. Caller passes the raw bytes from the
/// file picker. Returns the newly inserted chart's UUID plus any warnings
/// (missing indicators, unknown drawing kinds, etc.).
#[tauri::command]
pub async fn import_chart_xol(bytes: Vec<u8>) -> Result<ImportResult, String> {
    let pool = crate::drawing_db::get_pool()
        .ok_or_else(|| "DB not initialized".to_string())?;
    let (state, warnings) = xol::read(&bytes).map_err(|e| e.to_string())?;
    let chart_id = save_with_warnings(pool, &state, &warnings)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ImportResult {
        chart_id: chart_id.to_string(),
        warnings,
    })
}

/// Save a chart to a user-chosen `.xol` file via the system Save dialog.
/// Returns the chosen path as a string, or empty string if cancelled.
#[tauri::command]
pub async fn save_chart_to_file(chart_id: String) -> Result<String, String> {
    let pool = crate::drawing_db::get_pool()
        .ok_or_else(|| "DB not initialized".to_string())?;
    let id = Uuid::parse_str(&chart_id).map_err(|e| format!("invalid uuid: {e}"))?;
    let state = db::load_chart(pool, id).await.map_err(|e| e.to_string())?;
    let suggested = state
        .title
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_else(|| state.symbol.canonical.to_string());

    // rfd dialog blocks; off-load to a sync task.
    let saved = tokio::task::spawn_blocking(move || file_io::save_chart_dialog(&state, &suggested))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(saved.map(|p| p.display().to_string()).unwrap_or_default())
}

/// Show an Open dialog, load the selected `.xol` file, save it as a new
/// chart in the DB. Returns the new chart UUID + warnings, or
/// `chart_id == ""` if the user cancelled the dialog.
#[tauri::command]
pub async fn load_chart_from_file() -> Result<ImportResult, String> {
    let pool = crate::drawing_db::get_pool()
        .ok_or_else(|| "DB not initialized".to_string())?;
    let result = tokio::task::spawn_blocking(file_io::open_chart_dialog)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let Some((state, warnings)) = result else {
        return Ok(ImportResult { chart_id: String::new(), warnings: xol::ImportWarnings::default() });
    };
    let chart_id = save_with_warnings(pool, &state, &warnings)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ImportResult {
        chart_id: chart_id.to_string(),
        warnings,
    })
}

/// Save the chart and stamp non-blocking warnings into `import_warnings`.
async fn save_with_warnings(
    pool: &PgPool,
    state: &super::ChartState,
    warnings: &xol::ImportWarnings,
) -> Result<Uuid, db::DbCodecError> {
    let chart_id = db::save_chart(pool, state).await?;

    // Each warning gets a row so the user can review them later in a
    // dedicated UI panel without rerunning the import.
    for ref_id in &warnings.missing_indicators {
        let _ = sqlx::query(
            "INSERT INTO import_warnings (chart_id, kind, ref_id, detail) VALUES ($1, 1, $2, NULL)",
        )
        .bind(chart_id)
        .bind(ref_id)
        .execute(pool)
        .await;
    }
    for kind in &warnings.unknown_drawing_kinds {
        let _ = sqlx::query(
            "INSERT INTO import_warnings (chart_id, kind, ref_id, detail) VALUES ($1, 4, NULL, $2)",
        )
        .bind(chart_id)
        .bind(serde_json::json!({ "drawing_kind": kind }))
        .execute(pool)
        .await;
    }
    for (ref_id, version) in &warnings.unknown_indicator_param_versions {
        let _ = sqlx::query(
            "INSERT INTO import_warnings (chart_id, kind, ref_id, detail) VALUES ($1, 2, $2, $3)",
        )
        .bind(chart_id)
        .bind(ref_id)
        .bind(serde_json::json!({ "param_schema_version": version }))
        .execute(pool)
        .await;
    }

    Ok(chart_id)
}
