//! On-screen frame profiler overlay. Toggle with Ctrl+Shift+P.
//!
//! Reads from `crate::monitoring::current_snapshot()` and renders a small
//! window in the top-right showing frame timings, phase breakdown, subsystem
//! spans, allocation counts, GPU/CPU/RAM stats, and recent jank events.

use egui::{Color32, RichText};
use super::super::style::{font_xs, font_sm, color_alpha, ALPHA_SOLID};
fn ft() -> &'static crate::chart_renderer::gpu::Theme { &crate::chart_renderer::gpu::THEMES[0] }

fn us_to_ms(us: u64) -> f64 { us as f64 / 1000.0 }

/// Color a value: green if fast, yellow if moderate, red if slow.
fn phase_color(us: u64, warn_us: u64, bad_us: u64) -> Color32 {
    if us >= bad_us       { ft().bear }
    else if us >= warn_us { ft().warn }
    else                  { ft().bull }
}

/// Render a sparkline of frame times in a tiny painter strip.
fn sparkline(ui: &mut egui::Ui, values: &[f64], width: f32, height: f32) {
    if values.is_empty() { return; }
    let (min_v, max_v) = values.iter().fold((f64::MAX, 0_f64), |(mn, mx), &v| (mn.min(v), mx.max(v)));
    let range = (max_v - min_v).max(1.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let n = values.len();
    let bar_w = (width / n as f32).max(1.0);
    for (i, &v) in values.iter().enumerate() {
        let norm = ((v - min_v) / range) as f32;
        let bar_h = (norm * height).max(1.0);
        let x = rect.left() + i as f32 * bar_w;
        let col = phase_color((v * 1000.0) as u64, 16_000, 33_000);
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, rect.bottom() - bar_h), egui::vec2(bar_w - 0.5, bar_h)),
            0.0, col,
        );
    }
}

/// Toggle-able perf overlay. Call once per frame after all panels.
/// `open` is read and written — caller mirrors it to an AtomicBool.
pub fn show(ctx: &egui::Context, open: &mut bool) {
    if !*open { return; }

    let snap = crate::monitoring::current_snapshot();

    // Build a short sparkline from per-frame ring (we only have avg — use last 1 frame placeholder)
    // We use subsystem stats for the sparkline placeholder using fps history approximation.
    let fps = snap.frames.fps;
    let frame_ms = us_to_ms(snap.frames.last_frame_us);
    let avg_ms   = us_to_ms(snap.frames.avg_frame_us);

    egui::Window::new("⏱ Perf HUD")
        .id(egui::Id::new("perf_hud_window"))
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
        .resizable(true)
        .collapsible(true)
        .default_width(300.0)
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(color_alpha(ft().bg, ALPHA_SOLID))
                .stroke(egui::Stroke::new(1.0, ft().toolbar_border))
                .inner_margin(8.0)
                .corner_radius(4.0),
        )
        .open(open)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let t     = ft();
            let dim   = t.dim;
            let white = t.text;
            let warn  = t.warn;
            let red   = t.bear;
            let green = t.bull;
            let label_font = egui::FontId::monospace(font_xs());
            let val_font   = egui::FontId::monospace(font_sm());

            // ── Frame summary ──────────────────────────────────────────────
            let fps_col = if fps >= 55.0 { green } else if fps >= 30.0 { warn } else { red };
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Frame {:.1}ms", frame_ms))
                    .font(val_font.clone()).color(white).strong());
                ui.label(RichText::new(format!("({:.0} fps)", fps))
                    .font(val_font.clone()).color(fps_col));
                ui.label(RichText::new(format!("avg {:.1}ms", avg_ms))
                    .font(label_font.clone()).color(dim));
            });

            // Mini sparkline (60 ticks wide, 12 px tall) — filled with fps-derived values
            // We approximate with a short history using last/avg alternation
            let spark_vals: Vec<f64> = {
                let last = snap.frames.last_frame_us;
                let avg  = snap.frames.avg_frame_us;
                let p99  = snap.frames.p99_frame_us;
                vec![avg as f64, avg as f64, last as f64, avg as f64, p99 as f64,
                     avg as f64, last as f64, avg as f64, last as f64, avg as f64]
            };
            sparkline(ui, &spark_vals, 120.0, 14.0);

            ui.add_space(4.0);
            ui.separator();

            // ── Frame phase breakdown ──────────────────────────────────────
            ui.label(RichText::new("phases (ms)").font(label_font.clone()).color(dim));
            let p = &snap.phases;
            let phases = [
                ("acq",     p.avg_acquire_us,     2_000,  10_000),
                ("layout",  p.avg_layout_us,      8_000,  20_000),
                ("tess",    p.avg_tessellate_us,  4_000,  10_000),
                ("upload",  p.avg_upload_us,      2_000,   6_000),
                ("render",  p.avg_render_us,      4_000,  10_000),
                ("present", p.avg_present_us,     8_000,  20_000),
            ];
            // Find max phase for highlight
            let max_phase_us = phases.iter().map(|&(_, v, _, _)| v).max().unwrap_or(0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for &(name, us, wt, bt) in &phases {
                    let col = if us == max_phase_us && us > 2_000 { red } else { phase_color(us, wt, bt) };
                    ui.label(RichText::new(format!("{} {:.1}", name, us_to_ms(us)))
                        .font(label_font.clone()).color(col));
                }
            });

            ui.add_space(4.0);
            ui.separator();

            // ── Subsystem span breakdown ───────────────────────────────────
            if !snap.subsystems.spans.is_empty() {
                ui.label(RichText::new("subsystems (ms)").font(label_font.clone()).color(dim));
                // Show top spans by last_us descending
                let mut sorted = snap.subsystems.spans.clone();
                sorted.sort_by(|a, b| b.3.cmp(&a.3)); // sort by last_us desc
                for (name, avg_us, max_us, last_us) in sorted.iter().take(10) {
                    let col = phase_color(*last_us, 4_000, 12_000);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("  {:<20}", name))
                            .font(label_font.clone()).color(dim));
                        ui.label(RichText::new(format!("{:.1}", us_to_ms(*last_us)))
                            .font(label_font.clone()).color(col));
                        ui.label(RichText::new(format!("avg {:.1} max {:.1}",
                            us_to_ms(*avg_us), us_to_ms(*max_us)))
                            .font(label_font.clone()).color(dim));
                    });
                }

                ui.add_space(4.0);
                ui.separator();
            }

            // ── Allocations ───────────────────────────────────────────────
            let a = &snap.allocs;
            let alloc_kb = a.frame_alloc_bytes as f64 / 1024.0;
            let alloc_col = if a.frame_allocs > 500 { warn } else { green };
            ui.label(RichText::new(format!("alloc: {} calls / {:.1} KB", a.frame_allocs, alloc_kb))
                .font(label_font.clone()).color(alloc_col));

            ui.add_space(4.0);
            ui.separator();

            // ── GPU / CPU / RAM ────────────────────────────────────────────
            if let Some(gpu) = snap.gpus.first() {
                let vram_used_gb = gpu.memory_used as f64 / 1_073_741_824.0;
                let vram_tot_gb  = gpu.memory_total as f64 / 1_073_741_824.0;
                let gpu_col = if gpu.utilization_gpu > 80 { warn } else { green };
                ui.label(RichText::new(format!("gpu: {}% / vram {:.1}/{:.1} GB  {}°C",
                    gpu.utilization_gpu, vram_used_gb, vram_tot_gb, gpu.temperature))
                    .font(label_font.clone()).color(gpu_col));
            } else {
                ui.label(RichText::new("gpu: n/a").font(label_font.clone()).color(dim));
            }
            let sys = &snap.system;
            let used_ram_gb = sys.used_memory as f64 / 1_073_741_824.0;
            let tot_ram_gb  = sys.total_memory as f64 / 1_073_741_824.0;
            let proc = &snap.process;
            let cpu_col = if proc.cpu_percent > 80.0 { warn } else { green };
            ui.label(RichText::new(format!("cpu: {:.0}% / ram {:.1}/{:.1} GB",
                proc.cpu_percent, used_ram_gb, tot_ram_gb))
                .font(label_font.clone()).color(cpu_col));

            // ── Jank events ────────────────────────────────────────────────
            if !snap.jank_events.is_empty() {
                ui.add_space(4.0);
                ui.separator();
                ui.label(RichText::new(format!("jank: {} recent events", snap.jank_events.len()))
                    .font(label_font.clone()).color(red));
                for ev in snap.jank_events.iter().rev().take(3) {
                    ui.label(RichText::new(format!("  frame#{} {:.1}ms  alloc {}",
                        ev.frame_number, us_to_ms(ev.total_us), ev.allocs_in_frame))
                        .font(label_font.clone()).color(warn));
                }
            }

            ui.add_space(4.0);
            ui.label(RichText::new("Ctrl+Shift+P to close").font(label_font.clone()).color(dim));
        });
}
