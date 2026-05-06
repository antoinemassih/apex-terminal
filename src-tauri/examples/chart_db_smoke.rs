//! Live round-trip smoke test against the new chart_state schema.
//!
//! Run with:
//!   cargo run --example chart_db_smoke
//!
//! Connects to the Postgres at 192.168.1.143, saves a small chart with a
//! trendline + annotation + indicator, loads it back, and asserts equality.

use _scaffold_lib::chart_state::{
    annotations::Annotation,
    codec::db::{load_chart, save_chart},
    drawings::{Drawing, DrawingFlags, DrawingKind, Point},
    indicators::IndicatorRef,
    style_table::{DashKind, Style},
    AssetClass, ChartState, ProviderHints, Symbol, ThemeOverride, Timeframe,
};
use smallvec::smallvec;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect("postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo")
        .await?;

    // Build a tiny chart
    let mut state = ChartState::new(
        0,
        Symbol {
            canonical: "SPX".into(),
            asset_class: AssetClass::Index,
            provider_hints: ProviderHints::default(),
        },
        Timeframe::M5,
    );
    state.title = Some("smoke test".into());
    state.theme = ThemeOverride::Dark;
    state.viewport = _scaffold_lib::chart_state::Viewport {
        from_ts_ns: 1_700_000_000_000_000_000,
        to_ts_ns: 1_700_000_900_000_000_000,
        price_low: 4500.0,
        price_high: 4600.0,
        log_scale: false,
    };

    let style_id = state.style_table.intern(Style {
        stroke: 0xFFB800CC,
        width_x100: 150,
        dash: DashKind::Solid,
        fill: 0,
    });

    state.drawings.insert(Drawing {
        kind: DrawingKind::Trendline,
        points: smallvec![
            Point { ts_ns: 1_700_000_000_000_000_000, price: 4520.5 },
            Point { ts_ns: 1_700_000_300_000_000_000, price: 4555.0 },
        ],
        style: style_id,
        flags: DrawingFlags::VISIBLE | DrawingFlags::EXTEND_RIGHT,
        z: 100,
        extras: Default::default(),
    });

    state.annotations.insert(Annotation {
        anchor: Point { ts_ns: 1_700_000_500_000_000_000, price: 4540.0 },
        title: "gamma flip".into(),
        body_md: "watch for unwind".into(),
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

    println!("saving chart…");
    let id = save_chart(&pool, &state).await?;
    println!("saved chart {id}");

    println!("loading chart…");
    let loaded = load_chart(&pool, id).await?;

    assert_eq!(loaded.symbol.canonical.as_str(), "SPX");
    assert_eq!(loaded.symbol.asset_class, AssetClass::Index);
    assert_eq!(loaded.timeframe, Timeframe::M5);
    assert_eq!(loaded.theme, ThemeOverride::Dark);
    assert_eq!(loaded.title.as_ref().map(|s| s.as_str()), Some("smoke test"));
    assert_eq!(loaded.drawings.len(), 1);
    assert_eq!(loaded.annotations.len(), 1);
    assert_eq!(loaded.indicators.len(), 1);
    assert_eq!(loaded.style_table.len(), 1);

    let (_, d) = loaded.drawings.iter().next().unwrap();
    assert_eq!(d.kind, DrawingKind::Trendline);
    assert_eq!(d.points.len(), 2);
    assert_eq!(d.points[0].ts_ns, 1_700_000_000_000_000_000);
    assert!((d.points[0].price - 4520.5).abs() < 1e-3);
    assert!(d.flags.contains(DrawingFlags::EXTEND_RIGHT));

    let (_, a) = loaded.annotations.iter().next().unwrap();
    assert_eq!(a.title.as_str(), "gamma flip");
    assert_eq!(a.color, 0x22C55EFF);

    let ind = &loaded.indicators[0];
    assert_eq!(ind.ref_id.as_str(), "apex.vwap");
    assert_eq!(ind.params["session"], "rth");

    println!("✓ round-trip OK — chart {id} preserved everything");

    // Cleanup
    sqlx::query("DELETE FROM charts WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await?;
    println!("cleaned up");

    Ok(())
}
