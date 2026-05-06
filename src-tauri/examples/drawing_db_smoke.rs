//! End-to-end test of the renderer's persistence path.
//!
//! Exercises `drawing_db::save` → worker thread → new schema → `load_symbol`
//! to confirm the Stage A cutover preserves the public API contract while
//! talking to the new tables.
//!
//! Run with:  cargo run --example drawing_db_smoke

use _scaffold_lib::drawing_db::{self, DbDrawing};
use sqlx::postgres::PgPoolOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let pool = rt.block_on(async {
        PgPoolOptions::new()
            .max_connections(2)
            .connect("postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo")
            .await
    })?;

    drawing_db::init(pool.clone());
    // Give the worker a moment to spin up its runtime.
    std::thread::sleep(std::time::Duration::from_millis(150));

    let symbol = "SMOKE_DRAWING_TEST";
    let id1 = uuid::Uuid::new_v4().to_string();
    let id2 = uuid::Uuid::new_v4().to_string();

    let trendline = DbDrawing {
        id: id1.clone(),
        symbol: symbol.into(),
        timeframe: "5m".into(),
        drawing_type: "trendline".into(),
        points: vec![
            (1_700_000_000.0, 4520.5),
            (1_700_000_300.0, 4555.0),
        ],
        color: "#FFB800".into(),
        opacity: 0.8,
        line_style: "dashed".into(),
        thickness: 1.5,
        group_id: "trade-setup-7".into(),
    };

    let rect = DbDrawing {
        id: id2.clone(),
        symbol: symbol.into(),
        timeframe: "5m".into(),
        drawing_type: "rect".into(),
        points: vec![
            (1_700_000_400.0, 4500.0),
            (1_700_000_700.0, 4540.0),
        ],
        color: "#3B82F6".into(),
        opacity: 0.5,
        line_style: "solid".into(),
        thickness: 1.0,
        group_id: "default".into(),
    };

    println!("saving 2 drawings…");
    drawing_db::save(&trendline);
    drawing_db::save(&rect);

    // Saves are fire-and-forget; let the worker drain.
    std::thread::sleep(std::time::Duration::from_millis(400));

    println!("loading by symbol…");
    let loaded = drawing_db::load_symbol(symbol);
    assert_eq!(loaded.len(), 2, "expected 2 drawings, got {}", loaded.len());

    let by_id: std::collections::HashMap<_, _> =
        loaded.into_iter().map(|d| (d.id.clone(), d)).collect();

    let t = by_id.get(&id1).expect("trendline missing");
    assert_eq!(t.drawing_type, "trendline");
    assert_eq!(t.line_style, "dashed");
    assert!((t.thickness - 1.5).abs() < 1e-3);
    assert!((t.opacity - 0.8).abs() < 0.01, "opacity round-trip: {}", t.opacity);
    assert_eq!(t.color.to_uppercase(), "#FFB800");
    assert_eq!(t.group_id, "trade-setup-7");
    assert_eq!(t.points.len(), 2);
    assert!((t.points[0].0 - 1_700_000_000.0).abs() < 1.0);
    assert!((t.points[0].1 - 4520.5).abs() < 1e-3);

    let r = by_id.get(&id2).expect("rect missing");
    assert_eq!(r.drawing_type, "rect");
    assert_eq!(r.group_id, "default");

    println!("removing 1…");
    drawing_db::remove(&id1);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let after = drawing_db::load_symbol(symbol);
    assert_eq!(after.len(), 1, "expected 1 after remove");
    assert_eq!(after[0].id, id2);

    println!("✓ Stage A cutover OK — same public API, new schema");

    // Cleanup
    drawing_db::remove(&id2);
    std::thread::sleep(std::time::Duration::from_millis(200));
    rt.block_on(async {
        let _ = sqlx::query("DELETE FROM charts WHERE symbol_canonical = $1")
            .bind(symbol)
            .execute(&pool)
            .await;
    });
    println!("cleaned up");
    Ok(())
}
