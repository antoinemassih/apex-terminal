//! Design contracts, layout snapshots, and checkpoint serialisation.

use crate::dev_inspector::{SerRect, WidgetRecord};

// ─── Design contract ──────────────────────────────────────────────────────────

/// Builder for a set of geometric constraints on a single widget.
/// Violations are collected by `DevInspector::check_contract()` every frame.
#[derive(Default, Clone)]
pub struct Contract {
    min_w:      Option<f32>,
    min_h:      Option<f32>,
    max_w:      Option<f32>,
    max_h:      Option<f32>,
    touch:      Option<f32>,
    aspect_min: Option<f32>,
    aspect_max: Option<f32>,
    min_vis:    Option<f32>,
    non_empty:  bool,
    container:  Option<SerRect>,
}

impl Contract {
    pub fn new() -> Self { Self::default() }

    pub fn min_size(mut self, w: f32, h: f32) -> Self {
        self.min_w = Some(w); self.min_h = Some(h); self
    }
    pub fn max_size(mut self, w: f32, h: f32) -> Self {
        self.max_w = Some(w); self.max_h = Some(h); self
    }
    pub fn touch_target(mut self, min_px: f32) -> Self {
        self.touch = Some(min_px); self
    }
    pub fn aspect_ratio(mut self, min: f32, max: f32) -> Self {
        self.aspect_min = Some(min); self.aspect_max = Some(max); self
    }
    pub fn min_visible(mut self, min_area: f32) -> Self {
        self.min_vis = Some(min_area); self
    }
    pub fn non_empty(mut self) -> Self {
        self.non_empty = true; self
    }
    pub fn contained_in(mut self, container: egui::Rect) -> Self {
        self.container = Some(container.into()); self
    }

    /// Evaluate the contract against `rect`. Returns `(constraint_name, detail)` for each violation.
    pub fn check(&self, _id: &str, rect: egui::Rect) -> Vec<(String, String)> {
        let mut v = Vec::new();
        let r: SerRect = rect.into();

        if self.non_empty && r.area() == 0.0 {
            v.push(("non_empty".into(), format!("rect has zero area ({r:?})")));
        }
        if let Some(mw) = self.min_w {
            if r.w < mw { v.push(("min_size".into(), format!("w={:.1} < min {mw}", r.w))); }
        }
        if let Some(mh) = self.min_h {
            if r.h < mh { v.push(("min_size".into(), format!("h={:.1} < min {mh}", r.h))); }
        }
        if let Some(mw) = self.max_w {
            if r.w > mw { v.push(("max_size".into(), format!("w={:.1} > max {mw}", r.w))); }
        }
        if let Some(mh) = self.max_h {
            if r.h > mh { v.push(("max_size".into(), format!("h={:.1} > max {mh}", r.h))); }
        }
        if let Some(t) = self.touch {
            let ms = r.min_side();
            if ms < t { v.push(("touch_target".into(), format!("min_side={ms:.1} < {t}"))); }
        }
        if let Some(amin) = self.aspect_min {
            if r.h > 0.0 {
                let ar = r.w / r.h;
                if ar < amin {
                    v.push(("aspect_ratio".into(), format!("w/h={ar:.2} < min {amin}")));
                }
            }
        }
        if let Some(amax) = self.aspect_max {
            if r.h > 0.0 {
                let ar = r.w / r.h;
                if ar > amax {
                    v.push(("aspect_ratio".into(), format!("w/h={ar:.2} > max {amax}")));
                }
            }
        }
        if let Some(mv) = self.min_vis {
            if r.area() < mv {
                v.push(("min_visible".into(), format!("area={:.1} < min {mv}", r.area())));
            }
        }
        if let Some(ref c) = self.container {
            if !c.contains(&r) {
                v.push(("contained_in".into(), format!("rect {r:?} outside container {c:?}")));
            }
        }
        v
    }
}

// ─── Snapshot persistence ─────────────────────────────────────────────────────

const SNAPSHOT_DIR: &str = "dev/snapshots";
const CHECKPOINT_DIR: &str = "dev/checkpoints";

fn ensure_dir(path: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

/// Save current widget tree as a named golden snapshot.
pub fn save_snapshot(name: &str, widgets: &[WidgetRecord]) -> Result<String, String> {
    ensure_dir(SNAPSHOT_DIR).map_err(|e| e.to_string())?;
    let path = format!("{SNAPSHOT_DIR}/{name}.json");
    let json = serde_json::to_string_pretty(widgets).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Load a named snapshot. Returns the widget list or an error string.
pub fn load_snapshot(name: &str) -> Result<Vec<WidgetRecord>, String> {
    let path = format!("{SNAPSHOT_DIR}/{name}.json");
    let bytes = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// Diff the current widget tree against a named snapshot.
/// Returns a JSON summary of changes.
pub fn diff_layout(
    baseline_name: &str,
    current: &[WidgetRecord],
    threshold_px: f32,
    threshold_size_px: f32,
) -> serde_json::Value {
    let baseline = match load_snapshot(baseline_name) {
        Ok(b) => b,
        Err(e) => return serde_json::json!({"error": e}),
    };

    let mut regressions: Vec<serde_json::Value> = Vec::new();
    let mut appeared: Vec<String> = Vec::new();
    let mut disappeared: Vec<String> = Vec::new();

    // Check for disappeared widgets and regressions
    for bw in &baseline {
        match current.iter().find(|cw| cw.id == bw.id) {
            None => disappeared.push(bw.id.clone()),
            Some(cw) => {
                let dx = (cw.rect.x - bw.rect.x).abs();
                let dy = (cw.rect.y - bw.rect.y).abs();
                let dw = (cw.rect.w - bw.rect.w).abs();
                let dh = (cw.rect.h - bw.rect.h).abs();
                if dx > threshold_px || dy > threshold_px
                || dw > threshold_size_px || dh > threshold_size_px {
                    regressions.push(serde_json::json!({
                        "id": bw.id,
                        "pos_delta": [dx, dy],
                        "size_delta": [dw, dh],
                    }));
                }
            }
        }
    }

    // Check for appeared widgets
    for cw in current {
        if !baseline.iter().any(|bw| bw.id == cw.id) {
            appeared.push(cw.id.clone());
        }
    }

    serde_json::json!({
        "baseline":      baseline_name,
        "regressions":   regressions,
        "appeared":      appeared,
        "disappeared":   disappeared,
        "clean":         regressions.is_empty() && appeared.is_empty() && disappeared.is_empty(),
    })
}

/// Serialise full app state to a named checkpoint file.
pub fn save_checkpoint(name: &str, state: &serde_json::Value) -> Result<String, String> {
    ensure_dir(CHECKPOINT_DIR).map_err(|e| e.to_string())?;
    let path = format!("{CHECKPOINT_DIR}/{name}.json");
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

// ─── Scenario metadata ────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ScenarioMeta {
    pub name: String,
    pub file: String,
    pub step_count: usize,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

pub fn list_scenarios(dir: &str) -> Vec<ScenarioMeta> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") { continue; }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) else { continue };
        let name = val["name"].as_str().unwrap_or("unnamed").to_string();
        let file = path.file_name().and_then(|n| n.to_str())
            .unwrap_or("").to_string();
        let step_count = val["steps"].as_array().map(|a| a.len()).unwrap_or(0);
        let description = val["description"].as_str().map(|s| s.to_string());
        let tags = val["tags"].as_array().map(|a| {
            a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
        });
        out.push(ScenarioMeta { name, file, step_count, description, tags });
    }
    out.sort_by(|a, b| a.file.cmp(&b.file));
    out
}
