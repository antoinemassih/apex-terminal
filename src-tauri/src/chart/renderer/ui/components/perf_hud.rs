//! On-screen frame profiler overlay. Toggle with Ctrl+Shift+P.
//!
//! Reads from `crate::monitoring::current_snapshot()` and renders a small
//! window in the top-right showing frame timings, phase breakdown, subsystem
//! spans, allocation counts, GPU/CPU/RAM stats, and recent jank events.

use egui::{Color32, RichText};
use super::super::style::{color_alpha, alpha_solid, radius_sm, stroke_std, gap_xs, gap_sm};

#[inline(always)]
fn ambient(ctx: &egui::Context) -> crate::chart_renderer::gpu::Theme {
    crate::chart_renderer::theme_impl::active_theme(ctx)
}

fn us_to_ms(us: u64) -> f64 { us as f64 / 1000.0 }

/// Color a value: green if fast, yellow if moderate, red if slow.
fn phase_color(ctx: &egui::Context, us: u64, warn_us: u64, bad_us: u64) -> Color32 {
    let t = ambient(ctx);
    if us >= bad_us       { t.bear }
    else if us >= warn_us { t.warn }
    else                  { t.bull }
}

/// Render a sparkline of frame times in a tiny painter strip.
fn sparkline(ui: &mut egui::Ui, values: &[f64], width: f32, height: f32) {
    if values.is_empty() { return; }
    let ctx = ui.ctx().clone();
    let vals_f32: Vec<f32> = values.iter().map(|&v| v as f32).collect();
    let color_fn = move |v: f32| phase_color(&ctx, (v * 1000.0) as u64, 16_000, 33_000);
    let theme = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
    crate::ui_kit::widgets::Sparkline::new(&vals_f32)
        .bars()
        .bar_color(&color_fn)
        .size(width, height)
        .show(ui, &theme);
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

    // Dev tool — resolves theme from the ambient registry (active_theme).
    let amb = ambient(ctx);
    egui::Window::new("⏱ Perf HUD")
        .id(egui::Id::new("perf_hud_window"))
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
        .resizable(true)
        .collapsible(true)
        .default_width(300.0)
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(color_alpha(amb.bg, alpha_solid()))
                .stroke(egui::Stroke::new(stroke_std(), amb.toolbar_border))
                .inner_margin(gap_sm())
                .corner_radius(radius_sm()),
        )
        .open(open)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let t     = ambient(ui.ctx());
            let dim   = t.dim;
            let white = t.text;
            let warn  = t.warn;
            let red   = t.bear;
            let green = t.bull;
            let label_font = crate::ui_kit::style::mono_xs();
            let val_font   = crate::ui_kit::style::mono_sm();

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

            ui.add_space(gap_xs());
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
                    let col = if us == max_phase_us && us > 2_000 { red } else { phase_color(ui.ctx(), us, wt, bt) };
                    ui.label(RichText::new(format!("{} {:.1}", name, us_to_ms(us)))
                        .font(label_font.clone()).color(col));
                }
            });

            ui.add_space(gap_xs());
            ui.separator();

            // ── Subsystem span breakdown ───────────────────────────────────
            if !snap.subsystems.spans.is_empty() {
                ui.label(RichText::new("subsystems (ms)").font(label_font.clone()).color(dim));
                // Show top spans by last_us descending
                let mut sorted = snap.subsystems.spans.clone();
                sorted.sort_by(|a, b| b.3.cmp(&a.3)); // sort by last_us desc
                for (name, avg_us, max_us, last_us) in sorted.iter().take(10) {
                    let col = phase_color(ui.ctx(), *last_us, 4_000, 12_000);
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

                ui.add_space(gap_xs());
                ui.separator();
            }

            // ── Allocations ───────────────────────────────────────────────
            let a = &snap.allocs;
            let alloc_kb = a.frame_alloc_bytes as f64 / 1024.0;
            let alloc_col = if a.frame_allocs > 500 { warn } else { green };
            ui.label(RichText::new(format!("alloc: {} calls / {:.1} KB", a.frame_allocs, alloc_kb))
                .font(label_font.clone()).color(alloc_col));

            ui.add_space(gap_xs());
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

            // ── Frame profiler zones (CPU side, in-process) ───────────────
            ui.add_space(gap_xs());
            ui.separator();
            {
                use crate::foundation::frame_profiler;
                let recent = frame_profiler::recent_frames(60);
                let zones = frame_profiler::last_frame_zones();
                let last_total_us = recent.last().map(|f| f.total_us).unwrap_or(0);
                let last_total_ms = us_to_ms(last_total_us);
                let target_ms = 16.67_f64;
                let total_col = if last_total_ms < 12.0 { green }
                    else if last_total_ms < 20.0 { warn }
                    else { red };
                ui.horizontal(|ui| {
                    ui.label(RichText::new("profiler").font(label_font.clone()).color(dim));
                    ui.label(RichText::new(format!("{:.1}ms", last_total_ms))
                        .font(val_font.clone()).color(total_col).strong());
                    ui.label(RichText::new(format!("(target {:.2})", target_ms))
                        .font(label_font.clone()).color(dim));
                });

                // Frame-time history sparkline (last 60 frames).
                if !recent.is_empty() {
                    let vals: Vec<f64> = recent.iter().map(|f| us_to_ms(f.total_us)).collect();
                    sparkline(ui, &vals, 180.0, 14.0);
                }

                // Top 5 zones by duration from the last frame (depth=0 only,
                // so we don't double-count nested time).
                if !zones.is_empty() {
                    let mut top: Vec<&frame_profiler::ZoneSample> =
                        zones.iter().filter(|z| z.depth == 0).collect();
                    top.sort_by(|a, b| b.duration_us.cmp(&a.duration_us));
                    ui.label(RichText::new("top zones").font(label_font.clone()).color(dim));
                    for z in top.iter().take(5) {
                        let ms = us_to_ms(z.duration_us);
                        let col = if ms < 4.0 { green } else if ms < 10.0 { warn } else { red };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("  {:<18}", z.name))
                                .font(label_font.clone()).color(dim));
                            ui.label(RichText::new(format!("{:.2} ms", ms))
                                .font(label_font.clone()).color(col));
                        });
                    }
                }
            }

            // ── Jank events ────────────────────────────────────────────────
            if !snap.jank_events.is_empty() {
                ui.add_space(gap_xs());
                ui.separator();
                ui.label(RichText::new(format!("jank: {} recent events", snap.jank_events.len()))
                    .font(label_font.clone()).color(red));
                for ev in snap.jank_events.iter().rev().take(3) {
                    ui.label(RichText::new(format!("  frame#{} {:.1}ms  alloc {}",
                        ev.frame_number, us_to_ms(ev.total_us), ev.allocs_in_frame))
                        .font(label_font.clone()).color(warn));
                }
            }

            ui.add_space(gap_xs());
            ui.label(RichText::new("Ctrl+Shift+P to close").font(label_font.clone()).color(dim));
        });
}
