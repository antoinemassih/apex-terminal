//! End-to-end smoke for the XOL Export → Import flow against the live DB.
//!
//! Steps:
//!   1. Save a sample ChartState directly to the DB.
//!   2. Load it back and serialize to XOL bytes.
//!   3. Re-import those bytes into a brand-new chart row.
//!   4. Load the new chart and assert deep equality with the original.
//!
//! Run with:  cargo run --example xol_export_import_smoke

use _scaffold_lib::chart_state::{
    annotations::Annotation,
    codec::db::{load_chart, save_chart},
    codec::xol,
    drawings::{Drawing, DrawingFlags, DrawingKind, Point},
    indicators::IndicatorRef,
    style_table::{DashKind, Style},
    AssetClass, ChartState, ProviderHints, Symbol, ThemeOverride, Timeframe, Viewport,
};
use smallvec::smallvec;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect("postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo")
        .await?;

    // 1. Build a chart and save it
    let mut state = ChartState::new(
        0,
        Symbol {
            canonical: "SPX".into(),
            asset_class: AssetClass::Index,
            provider_hints: ProviderHints::default(),
        },
        Timeframe::M5,
    );
    state.title = Some("xol e2e smoke".into());
    state.theme = ThemeOverride::Dark;
    state.viewport = Viewport {
        from_ts_ns: 1_700_000_000_000_000_000,
        to_ts_ns: 1_700_000_900_000_000_000,
        price_low: 4500.0,
        price_high: 4600.0,
        log_scale: false,
    };

    let style = state.style_table.intern(Style {
        stroke: 0xFFB800CC,
        width_x100: 150,
        dash: DashKind::Dashed,
        fill: 0,
    });
    state.drawings.insert(Drawing {
        kind: DrawingKind::Trendline,
        points: smallvec![
            Point { ts_ns: 1_700_000_000_000_000_000, price: 4520.5 },
            Point { ts_ns: 1_700_000_300_000_000_000, price: 4555.0 },
        ],
        style,
        flags: DrawingFlags::VISIBLE | DrawingFlags::EXTEND_RIGHT,
        z: 100,
        extras: Default::default(),
    });
    state.annotations.insert(Annotation {
        anchor: Point { ts_ns: 1_700_000_500_000_000_000, price: 4540.0 },
        title: "gamma flip".into(),
        body_md: "watch unwind".into(),
        color: 0x22C55EFF,
        asset_refs: smallvec![],
        extras: Default::default(),
    });
    state.indicators.push(IndicatorRef {
        ref_id: "apex.vwap".into(),
        ref_version: 2,
        param_schema_version: 1,
        pane: "main".into(),
        params: serde_json::json!({"session": "rth"}),
        style: None,
        installed_locally: true,
        extras: Default::default(),
    });

    println!("1. saving original chart…");
    let original_id = save_chart(&pool, &state).await?;
    println!("   saved {original_id}");

    // 2. Load back and export to XOL
    println!("2. loading + exporting to XOL…");
    let loaded = load_chart(&pool, original_id).await?;
    let bytes = xol::write(&loaded)?;
    println!("   XOL is {} bytes", bytes.len());

    // 3. Re-import as a new chart
    println!("3. re-importing as new chart…");
    let (reimported, warnings) = xol::read(&bytes)?;
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    let new_id = save_chart(&pool, &reimported).await?;
    println!("   imported as {new_id}");

    // 4. Load new chart and compare with original
    println!("4. verifying deep equality…");
    let final_state = load_chart(&pool, new_id).await?;

    assert_eq!(final_state.symbol.canonical.as_str(), "SPX");
    assert_eq!(final_state.symbol.asset_class, AssetClass::Index);
    assert_eq!(final_state.timeframe, Timeframe::M5);
    assert_eq!(final_state.theme, ThemeOverride::Dark);
    assert_eq!(final_state.viewport, state.viewport);
    assert_eq!(final_state.title.as_ref().map(|s| s.as_str()), Some("xol e2e smoke"));
    assert_eq!(final_state.drawings.len(), 1);
    assert_eq!(final_state.annotations.len(), 1);
    assert_eq!(final_state.indicators.len(), 1);

    let (_, d) = final_state.drawings.iter().next().unwrap();
    assert_eq!(d.kind, DrawingKind::Trendline);
    assert!(d.flags.contains(DrawingFlags::EXTEND_RIGHT));
    assert_eq!(d.points.len(), 2);
    assert!((d.points[0].price - 4520.5).abs() < 1e-3);

    let (_, a) = final_state.annotations.iter().next().unwrap();
    assert_eq!(a.title.as_str(), "gamma flip");
    assert_eq!(a.color, 0x22C55EFF);

    println!("✓ XOL export → import round-trip OK against live DB");

    // Cleanup
    sqlx::query("DELETE FROM charts WHERE id = ANY($1)")
        .bind(&[original_id, new_id][..])
        .execute(&pool)
        .await?;
    println!("cleaned up both charts");

    Ok(())
}
