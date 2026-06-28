//! Canvas introspection — per-frame snapshot of chart drawing state.
//!
//! Provides pixel-accurate world ↔ screen coordinate snapshots for every
//! pane: drawings (price/time anchors → screen xy), indicator output series,
//! and the visible bar strip with OHLCV + screen positions.
//!
//! Performance contract: capture() is called from end_frame() (main thread,
//! after all GPU work is queued). It only reads already-computed state —
//! no FFT, no allocation beyond the snapshot itself. Debug builds only.

use serde::Serialize;

// ─── Public data types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Default)]
pub struct CanvasSnapshot {
    pub captured_at_frame: u64,
    pub active_pane: usize,
    pub panes: Vec<PaneCanvas>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaneCanvas {
    pub index: usize,
    pub symbol: String,
    pub timeframe: String,
    pub pane_type: String,
    pub viewport: CanvasViewport,
    pub screen_rect: ScreenRect,
    pub drawings: Vec<DrawingRecord>,
    pub indicators: Vec<IndicatorRecord>,
    /// Visible bars (left→right), limited to at most `viewport.visible_bar_count`.
    pub bars: Vec<BarRecord>,
    pub selected_drawing_id: Option<String>,
    pub active_draw_tool: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CanvasViewport {
    /// Visible price range bottom (world space).
    pub price_low: f32,
    /// Visible price range top (world space).
    pub price_high: f32,
    /// Timestamp (nanoseconds) of the leftmost visible bar.
    pub from_ts_ns: i64,
    /// Timestamp (nanoseconds) of the rightmost visible bar.
    pub to_ts_ns: i64,
    /// Number of bars currently fitting on screen.
    pub visible_bar_count: u32,
    pub log_scale: bool,
    /// Pixels per bar (horizontal density).
    pub px_per_bar: f32,
    /// Pixels per price unit (vertical density, linear scale).
    pub px_per_price: f32,
}

/// Screen rectangle in window pixels.
#[derive(Debug, Clone, Serialize, Default)]
pub struct ScreenRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DrawingRecord {
    /// Inspector-assigned ID ("drawing.0", "drawing.1", …).
    pub id: String,
    pub kind: String,
    pub visible: bool,
    pub locked: bool,
    pub selected: bool,
    /// Anchor points in world (price, time) space + derived screen xy.
    pub anchors: Vec<DrawingAnchor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DrawingAnchor {
    /// Unix nanoseconds. 0 in headless synthetic mode.
    pub ts_ns: i64,
    /// Price value.
    pub price: f32,
    /// Derived screen x (pixels from window left).
    pub screen_x: f32,
    /// Derived screen y (pixels from window top). Lower price → higher y.
    pub screen_y: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndicatorRecord {
    pub id: u32,
    pub kind: String,
    pub period: usize,
    pub visible: bool,
    /// Last computed value of the primary series (None if series is empty).
    pub last_value: Option<f32>,
    /// Last value of auxiliary series 2 (signal line, upper band, etc.).
    pub last_value2: Option<f32>,
    /// Last value of auxiliary series 3 (lower band, etc.).
    pub last_value3: Option<f32>,
    /// Min of primary series values in the visible window.
    pub visible_min: f32,
    /// Max of primary series values in the visible window.
    pub visible_max: f32,
    /// Total number of computed values.
    pub series_length: usize,
    /// Sparse sample of (bar, value, screen_xy) tuples for the visible range.
    pub samples: Vec<IndicatorSample>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndicatorSample {
    pub bar_index: usize,
    pub ts_ns: i64,
    pub value: f32,
    pub screen_x: f32,
    pub screen_y: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct BarRecord {
    /// Global bar index within the chart's full bar series.
    pub index: usize,
    pub ts_ns: i64,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    /// Screen x at bar center.
    pub screen_x: f32,
    pub screen_open_y: f32,
    pub screen_high_y: f32,
    pub screen_low_y: f32,
    pub screen_close_y: f32,
    pub is_bullish: bool,
}

// ─── Live-mode capture (main render thread) ───────────────────────────────────

/// Capture a full canvas snapshot from the live chart panes.
/// Called from `end_frame()` after all rendering work is queued for the GPU.
pub fn capture(
    panes: &[crate::chart_renderer::gpu::Chart],
    active_pane: usize,
) -> CanvasSnapshot {
    CanvasSnapshot {
        captured_at_frame: 0, // patched by caller with frame_counter
        active_pane,
        panes: panes.iter().enumerate().map(|(i, c)| capture_pane(i, c)).collect(),
    }
}

fn drawing_kind_name(kind: &crate::chart_renderer::DrawingKind) -> &'static str {
    use crate::chart_renderer::DrawingKind;
    match kind {
        DrawingKind::HLine { .. }              => "HorizLine",
        DrawingKind::TrendLine { .. }          => "Trendline",
        DrawingKind::HZone { .. }              => "HorizZone",
        DrawingKind::BarMarker { .. }          => "BarMarker",
        DrawingKind::Fibonacci { .. }          => "Fibonacci",
        DrawingKind::Channel { .. }            => "Channel",
        DrawingKind::FibChannel { .. }         => "FibChannel",
        DrawingKind::Pitchfork { .. }          => "Pitchfork",
        DrawingKind::GannFan { .. }            => "GannFan",
        DrawingKind::RegressionChannel { .. }  => "RegressionChannel",
        DrawingKind::XABCD { .. }              => "XABCD",
        DrawingKind::ElliottWave { .. }        => "ElliottWave",
        DrawingKind::AnchoredVWAP { .. }       => "AnchoredVWAP",
        DrawingKind::PriceRange { .. }         => "PriceRange",
        DrawingKind::RiskReward { .. }         => "RiskReward",
        DrawingKind::VerticalLine { .. }       => "VerticalLine",
        DrawingKind::Ray { .. }                => "Ray",
        DrawingKind::FibExtension { .. }       => "FibExtension",
        DrawingKind::FibTimeZone { .. }        => "FibTimeZone",
        DrawingKind::FibArc { .. }             => "FibArc",
        DrawingKind::GannBox { .. }            => "GannBox",
        DrawingKind::TextNote { .. }           => "TextNote",
    }
}

/// Extract (ts_ns, price) anchor points from a DrawingKind.
fn drawing_anchors(kind: &crate::chart_renderer::DrawingKind) -> Vec<(i64, f32)> {
    use crate::chart_renderer::DrawingKind;
    match kind {
        DrawingKind::HLine { price }         => vec![(0, *price)],
        DrawingKind::HZone { price0, price1} => vec![(0, *price0), (0, *price1)],
        DrawingKind::BarMarker { time, price, .. } => vec![(*time, *price)],
        DrawingKind::TrendLine { price0, time0, price1, time1 }
        | DrawingKind::Fibonacci { price0, time0, price1, time1 }
        | DrawingKind::GannFan  { price0, time0, price1, time1 }
        | DrawingKind::PriceRange { price0, time0, price1, time1 }
        | DrawingKind::FibArc   { price0, time0, price1, time1 }
        | DrawingKind::GannBox  { price0, time0, price1, time1 }
        | DrawingKind::Ray      { price0, time0, price1, time1 }
        => vec![(*time0, *price0), (*time1, *price1)],
        DrawingKind::Channel    { price0, time0, price1, time1, .. }
        | DrawingKind::FibChannel { price0, time0, price1, time1, .. }
        => vec![(*time0, *price0), (*time1, *price1)],
        DrawingKind::Pitchfork  { price0, time0, price1, time1, price2, time2 }
        | DrawingKind::FibExtension { price0, time0, price1, time1, price2, time2 }
        => vec![(*time0, *price0), (*time1, *price1), (*time2, *price2)],
        DrawingKind::RegressionChannel { time0, time1 } => vec![(*time0, 0.0), (*time1, 0.0)],
        DrawingKind::XABCD { points } | DrawingKind::ElliottWave { points, .. }
        => points.iter().map(|&(t, p)| (t, p)).collect(),
        DrawingKind::AnchoredVWAP { time }    => vec![(*time, 0.0)],
        DrawingKind::VerticalLine { time }    => vec![(*time, 0.0)],
        DrawingKind::FibTimeZone  { time }    => vec![(*time, 0.0)],
        DrawingKind::RiskReward { entry_price, entry_time, .. }
        => vec![(*entry_time, *entry_price)],
        DrawingKind::TextNote { price, time, .. } => vec![(*time, *price)],
    }
}

fn capture_pane(idx: usize, chart: &crate::chart_renderer::gpu::Chart) -> PaneCanvas {
    let n = chart.bars.len();
    let vs_int = chart.vs.floor() as usize;
    // vs_frac: fractional bar-pan offset from gpu params if available, else chart.vs.fract()
    #[cfg(feature = "gpu_chart_v2")]
    let vs_frac = chart.gpu_render_params.vs_frac;
    #[cfg(not(feature = "gpu_chart_v2"))]
    let vs_frac = chart.vs.fract();

    // Rightmost visible bar index (0-indexed, clamped).
    let right_bar = n.saturating_sub(vs_int + 1);
    let left_bar  = right_bar.saturating_sub(chart.vc as usize);

    // Price range: prefer price_lock (user-set), else compute from visible bars.
    let (price_low, price_high) = chart.price_lock.unwrap_or_else(|| {
        if n == 0 { return (0.0, 1.0); }
        let visible = &chart.bars[left_bar..=right_bar.min(n - 1)];
        let lo = visible.iter().map(|b| b.low).fold(f32::MAX, f32::min);
        let hi = visible.iter().map(|b| b.high).fold(f32::MIN, f32::max);
        (lo.max(0.0), hi.max(lo + 0.01))
    });

    // Timestamps of the visible range edges.
    let from_ts_ns = chart.timestamps.get(left_bar).copied().unwrap_or(0);
    let to_ts_ns   = chart.timestamps.get(right_bar).copied().unwrap_or(0);

    // Screen rect and px-density from GPU params (gpu_chart_v2) or synthetic fallback.
    #[cfg(feature = "gpu_chart_v2")]
    let (screen_rect, px_per_bar, vc_total_f) = {
        let r = &chart.gpu_render_params.chart_rect;
        let w = (r[2] - r[0]).max(1.0);
        let h = (r[3] - r[1]).max(1.0);
        let vct = chart.gpu_render_params.vc_total.max(1.0);
        (ScreenRect { x: r[0], y: r[1], w, h }, w / vct, vct)
    };
    #[cfg(not(feature = "gpu_chart_v2"))]
    let (screen_rect, px_per_bar, vc_total_f) = {
        let vc = (chart.vc as f32).max(1.0);
        (ScreenRect { x: 0.0, y: 0.0, w: 1200.0, h: 600.0 }, 1200.0 / vc, vc)
    };

    let price_range = (price_high - price_low).max(0.001);
    let px_per_price = screen_rect.h / price_range;

    let viewport = CanvasViewport {
        price_low, price_high,
        from_ts_ns, to_ts_ns,
        visible_bar_count: chart.vc,
        log_scale: chart.log_scale,
        px_per_bar,
        px_per_price,
    };

    // ── Coordinate helpers ────────────────────────────────────────────────────

    let price_to_y = |price: f32| -> f32 {
        if chart.log_scale && price_high > 0.0 && price_low > 0.0 && price > 0.0 {
            let lo_ln = price_low.ln();
            let hi_ln = price_high.ln();
            let p_ln  = price.ln();
            let range = (hi_ln - lo_ln).max(1e-9);
            screen_rect.y + (hi_ln - p_ln) / range * screen_rect.h
        } else {
            screen_rect.y + (price_high - price) / price_range * screen_rect.h
        }
    };

    let bar_to_x = |bar_index: usize| -> f32 {
        // slot_from_right: how many bars to the left of the right edge this bar is.
        let slot_from_right = (right_bar as i64 - bar_index as i64) as f32;
        // Right edge of visible area minus the distance in bar-widths, offset by vs_frac.
        let right_px = screen_rect.x + screen_rect.w;
        right_px - (slot_from_right - vs_frac + 0.5) * px_per_bar
    };

    // ts_ns → approximate screen_x (binary-search bar, then bar_to_x)
    let ts_to_x = |ts: i64| -> f32 {
        let bi = chart.timestamps.binary_search(&ts)
            .unwrap_or_else(|e| e.saturating_sub(1));
        bar_to_x(bi)
    };

    // ── Drawings ──────────────────────────────────────────────────────────────

    let drawings: Vec<DrawingRecord> = chart.drawings.iter().map(|d| {
        let raw = drawing_anchors(&d.kind);
        let anchors: Vec<DrawingAnchor> = raw.iter().map(|&(ts, price)| DrawingAnchor {
            ts_ns: ts, price,
            screen_x: ts_to_x(ts),
            screen_y: price_to_y(price),
        }).collect();
        let visible = !chart.hidden_groups.contains(&d.group_id);
        let selected = chart.selected_id.as_deref() == Some(d.id.as_str())
            || chart.selected_ids.iter().any(|s| s == &d.id);
        DrawingRecord {
            id: d.id.clone(),
            kind: drawing_kind_name(&d.kind).to_string(),
            visible,
            locked: d.locked,
            selected,
            anchors,
        }
    }).collect();

    // ── Indicators ────────────────────────────────────────────────────────────

    let indicators: Vec<IndicatorRecord> = chart.indicators.iter()
        .map(|ind| {
            let slen = ind.values.len();
            let last_value = ind.values.last().copied();
            let last_value2 = ind.values2.last().copied()
                .filter(|&v| v != 0.0);
            let last_value3 = ind.values3.last().copied()
                .filter(|&v| v != 0.0);

            // Visible min/max (primary series only, finite values).
            let vis_slice = if slen > 0 {
                &ind.values[left_bar.min(slen - 1)..=right_bar.min(slen - 1)]
            } else {
                &ind.values[..]
            };
            let visible_min = vis_slice.iter().cloned()
                .filter(|v| v.is_finite()).fold(f32::MAX, f32::min);
            let visible_max = vis_slice.iter().cloned()
                .filter(|v| v.is_finite()).fold(f32::MIN, f32::max);

            // Sparse samples: up to 20 evenly spaced bars in the visible range.
            let step = ((right_bar.saturating_sub(left_bar)) / 20).max(1);
            let samples: Vec<IndicatorSample> = (left_bar..=right_bar.min(slen.saturating_sub(1)))
                .step_by(step)
                .filter_map(|bi| {
                    let val = *ind.values.get(bi)?;
                    if !val.is_finite() { return None; }
                    Some(IndicatorSample {
                        bar_index: bi,
                        ts_ns:    chart.timestamps.get(bi).copied().unwrap_or(0),
                        value:    val,
                        screen_x: bar_to_x(bi),
                        screen_y: price_to_y(val),
                    })
                })
                .collect();

            IndicatorRecord {
                id:           ind.id,
                kind:         ind.kind.label().to_string(),
                period:       ind.period,
                visible:      ind.visible,
                last_value,
                last_value2,
                last_value3,
                visible_min:  if visible_min == f32::MAX { 0.0 } else { visible_min },
                visible_max:  if visible_max == f32::MIN { 0.0 } else { visible_max },
                series_length: slen,
                samples,
            }
        })
        .collect();

    // ── Visible bars ──────────────────────────────────────────────────────────

    // `.get(i)` + filter_map (not direct `chart.bars[i]`) so an empty-bars pane
    // — e.g. a symbol whose data 404s and never loads — can't panic the render
    // thread (which would close the window). Mirrors the indicator loop above.
    let bars: Vec<BarRecord> = if n == 0 {
        Vec::new()
    } else {
        (left_bar..=right_bar.min(n - 1))
            .filter_map(|i| {
                let b = chart.bars.get(i)?;
                Some(BarRecord {
                    index: i,
                    ts_ns:         chart.timestamps.get(i).copied().unwrap_or(0),
                    open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
                    screen_x:       bar_to_x(i),
                    screen_open_y:  price_to_y(b.open),
                    screen_high_y:  price_to_y(b.high),
                    screen_low_y:   price_to_y(b.low),
                    screen_close_y: price_to_y(b.close),
                    is_bullish:     b.close >= b.open,
                })
            })
            .collect()
    };

    PaneCanvas {
        index: idx,
        symbol:         chart.symbol.clone(),
        timeframe:      chart.timeframe.clone(),
        pane_type:      format!("{:?}", chart.pane_type),
        viewport,
        screen_rect,
        drawings,
        indicators,
        bars,
        selected_drawing_id: chart.selected_id.clone(),
        active_draw_tool: if chart.draw_tool.is_empty() { None }
                          else { Some(chart.draw_tool.clone()) },
    }
}

// ─── Headless synthetic capture ───────────────────────────────────────────────

/// Headless state needed to build a synthetic canvas snapshot.
/// Populated from HeadlessState fields added in this sprint.
pub struct HeadlessCanvasInput<'a> {
    pub panes: &'a [HeadlessPane<'a>],
    pub active_pane: usize,
}

pub struct HeadlessPane<'a> {
    pub index: usize,
    pub symbol: &'a str,
    pub timeframe: &'a str,
    pub pane_type: &'a str,
    /// Simulated visible bar count (synthetic, not real data).
    pub visible_bar_count: u32,
    pub price_low: f64,
    pub price_high: f64,
    pub drawings: &'a [HeadlessDrawing],
    pub indicators: &'a [HeadlessIndicator],
    /// Number of synthetic bars (used for bar assertions).
    pub bar_count: usize,
}

pub struct HeadlessDrawing {
    pub id: String,
    pub kind: String,
    /// Anchor A: (bar_offset_from_right: f64, price: f64). bar_offset=0 → rightmost bar.
    pub anchor_a: (f64, f64),
    /// Anchor B (optional second point).
    pub anchor_b: Option<(f64, f64)>,
    pub visible: bool,
    pub selected: bool,
}

pub struct HeadlessIndicator {
    pub id: u32,
    pub kind: String,
    pub period: usize,
    pub visible: bool,
    pub last_value: Option<f64>,
    pub last_value2: Option<f64>,
}

/// Build a CanvasSnapshot from synthetic headless state.
pub fn from_headless(input: &HeadlessCanvasInput<'_>) -> CanvasSnapshot {
    let panes = input.panes.iter().map(|p| {
        let price_low  = p.price_low  as f32;
        let price_high = (p.price_high as f32).max(price_low + 0.001);
        let price_range = (price_high - price_low).max(0.001);
        let vc = p.visible_bar_count.max(1) as f32;
        // Synthetic 1200×600 screen with no real GPU params.
        let sr = ScreenRect { x: 0.0, y: 0.0, w: 1200.0, h: 600.0 };
        let px_per_bar   = sr.w / vc;
        let px_per_price = sr.h / price_range;

        // Coordinate helpers (linear scale only in headless).
        let price_to_y = |price: f64| -> f32 {
            sr.y + (price_high - price as f32) / price_range * sr.h
        };
        // bar_offset: 0 = rightmost, positive = further left.
        let offset_to_x = |bar_offset: f64| -> f32 {
            sr.x + sr.w - (bar_offset as f32 + 0.5) * px_per_bar
        };

        let drawings: Vec<DrawingRecord> = p.drawings.iter().map(|d| {
            let mut anchors = vec![DrawingAnchor {
                ts_ns:    0,
                price:    d.anchor_a.1 as f32,
                screen_x: offset_to_x(d.anchor_a.0),
                screen_y: price_to_y(d.anchor_a.1),
            }];
            if let Some((off, pr)) = d.anchor_b {
                anchors.push(DrawingAnchor {
                    ts_ns: 0, price: pr as f32,
                    screen_x: offset_to_x(off),
                    screen_y: price_to_y(pr),
                });
            }
            DrawingRecord {
                id: d.id.clone(), kind: d.kind.clone(),
                visible: d.visible, locked: false, selected: d.selected,
                anchors,
            }
        }).collect();

        let indicators: Vec<IndicatorRecord> = p.indicators.iter().map(|ind| {
            let lv = ind.last_value.map(|v| v as f32);
            let visible_min = lv.unwrap_or(0.0);
            let visible_max = lv.unwrap_or(0.0);
            IndicatorRecord {
                id: ind.id, kind: ind.kind.clone(), period: ind.period,
                visible: ind.visible,
                last_value: lv,
                last_value2: ind.last_value2.map(|v| v as f32),
                last_value3: None,
                visible_min, visible_max,
                series_length: p.bar_count,
                samples: lv.map(|v| IndicatorSample {
                    bar_index: p.bar_count.saturating_sub(1),
                    ts_ns: 0, value: v,
                    screen_x: offset_to_x(0.0),
                    screen_y: price_to_y(ind.last_value.unwrap_or(0.0)),
                }).into_iter().collect(),
            }
        }).collect();

        // Synthetic bar data: evenly distributed price within the visible range.
        // Only create BarRecord entries if bar_count > 0.
        let bars: Vec<BarRecord> = if p.bar_count == 0 { vec![] } else {
            let synth_close = (price_low + price_high) / 2.0;
            let synth_open  = synth_close - price_range * 0.01;
            let synth_high  = synth_close + price_range * 0.02;
            let synth_low   = synth_close - price_range * 0.02;
            let total = p.bar_count;
            let visible_count = (p.visible_bar_count as usize).min(total).min(10);
            (total.saturating_sub(visible_count)..total)
                .enumerate()
                .map(|(slot, global_idx)| {
                    let offset = (visible_count - 1 - slot) as f64;
                    BarRecord {
                        index: global_idx, ts_ns: 0,
                        open: synth_open, high: synth_high, low: synth_low, close: synth_close,
                        volume: 1_000_000.0,
                        screen_x: offset_to_x(offset),
                        screen_open_y: price_to_y(synth_open as f64),
                        screen_high_y: price_to_y(synth_high as f64),
                        screen_low_y: price_to_y(synth_low as f64),
                        screen_close_y: price_to_y(synth_close as f64),
                        is_bullish: true,
                    }
                })
                .collect()
        };

        PaneCanvas {
            index: p.index, symbol: p.symbol.to_owned(), timeframe: p.timeframe.to_owned(),
            pane_type: p.pane_type.to_owned(),
            viewport: CanvasViewport {
                price_low, price_high,
                from_ts_ns: 0, to_ts_ns: 0,
                visible_bar_count: p.visible_bar_count,
                log_scale: false,
                px_per_bar, px_per_price,
            },
            screen_rect: sr,
            drawings, indicators, bars,
            selected_drawing_id: None,
            active_draw_tool: None,
        }
    }).collect();

    CanvasSnapshot { captured_at_frame: 0, active_pane: input.active_pane, panes }
}
