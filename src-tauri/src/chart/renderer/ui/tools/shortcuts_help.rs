//! Keyboard-shortcut cheatsheet overlay (WS-G G4).
//!
//! First real CONSUMER of the `foundation::shortcuts` registry's display API
//! (`by_category()` + `format_for_kbd()`), which was built to power a help
//! screen but had no UI until now. Renders every registered global shortcut,
//! grouped by category, through the canonical `ui_kit::Modal` shell + the
//! `Kbd` keycap widget — so what's shown here is always exactly what the
//! registry knows about (single source of truth for bindings).
//!
//! Open-state lives in ephemeral egui memory (a reference overlay needn't
//! persist across launches), so this adds NO field to the app god-struct and
//! no save/load plumbing. Toggle with F1; close with Esc / X / click-outside.

use super::super::style::*;
use super::super::super::gpu::*;
use crate::ui_kit::widgets::{Kbd, PanelSection};

fn open_id() -> egui::Id { egui::Id::new("shortcuts_help_open") }

/// Is the cheatsheet currently shown?
pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(open_id()).unwrap_or(false))
}

fn set_open(ctx: &egui::Context, v: bool) {
    ctx.data_mut(|d| d.insert_temp(open_id(), v));
}

/// Flip visibility. Called once per frame from the active-pane shortcut
/// handler (F1), so a plain toggle is race-free.
pub fn toggle(ctx: &egui::Context) {
    let now = is_open(ctx);
    set_open(ctx, !now);
}

/// Render the cheatsheet if open. Cheap no-op when closed. Call once per
/// frame from the top-nav overlay pass.
pub fn draw(ctx: &egui::Context, t: &Theme) {
    if !is_open(ctx) { return; }

    use crate::ui_kit::widgets::modal::{Modal, Anchor, HeaderStyle, FrameKind};

    // Snapshot the registry into owned, deterministically-ordered rows while
    // holding the read lock; `&'static str` fields outlive the guard and the
    // chords are owned Strings. Sort categories alphabetically; keep
    // registration order within each category.
    // Fallible read (`.ok()`), not a panicking unwrap: on the (practically
    // impossible) poisoned lock we render an empty cheatsheet, never panic in UI.
    let cats: Vec<(&'static str, Vec<(String, &'static str)>)> =
        crate::foundation::shortcuts::registry().read().ok().map(|reg| {
            let mut v: Vec<(&'static str, Vec<(String, &'static str)>)> = reg
                .by_category()
                .into_iter()
                .map(|(cat, entries)| {
                    let rows: Vec<(String, &'static str)> = entries
                        .iter()
                        .map(|e| (
                            crate::foundation::shortcuts::ShortcutRegistry::format_for_kbd(e.shortcut),
                            e.description,
                        ))
                        .collect();
                    (cat, rows)
                })
                .collect();
            v.sort_by(|a, b| a.0.cmp(b.0));
            v
        }).unwrap_or_default();

    let mut close = false;
    let resp = Modal::new("KEYBOARD SHORTCUTS")
        .id("shortcuts_help")
        .ctx(ctx)
        .theme(t)
        .size(egui::vec2(380.0, 0.0))
        .anchor(Anchor::Window { pos: None })
        .frame_kind(FrameKind::DialogWindow)
        .header_style(HeaderStyle::Dialog)
        .separator(true)
        .show(|ui| {
            ui.add_space(gap_sm());
            if cats.is_empty() {
                ui.label("No shortcuts registered yet.");
            }
            for (cat, rows) in &cats {
                PanelSection::new(*cat).show(ui, t, |ui, t| {
                    egui::Grid::new(format!("sc_grid_{cat}"))
                        .num_columns(2)
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            for (chord, desc) in rows {
                                Kbd::new(chord.clone()).show(ui, t);
                                ui.label(egui::RichText::new(*desc).color(t.text).size(font_sm()));
                                ui.end_row();
                            }
                        });
                });
                ui.add_space(gap_sm());
            }
        });

    if resp.closed { close = true; }
    // Esc also closes (Modal already handles the X + click-outside).
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { close = true; }
    if close { set_open(ctx, false); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_state_defaults_closed_and_toggles() {
        let ctx = egui::Context::default();
        // Fresh context: cheatsheet is closed.
        assert!(!is_open(&ctx), "should default to closed");
        // Toggle opens, toggle closes — race-free because the caller is
        // active-pane-gated (once per frame).
        toggle(&ctx);
        assert!(is_open(&ctx), "toggle should open");
        toggle(&ctx);
        assert!(!is_open(&ctx), "second toggle should close");
        // Explicit set_open round-trips.
        set_open(&ctx, true);
        assert!(is_open(&ctx));
        set_open(&ctx, false);
        assert!(!is_open(&ctx));
    }
}
