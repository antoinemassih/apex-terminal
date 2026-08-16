//! Settings panel — organized into Appearance, Chart, Trading, Notifications, Shortcuts tabs.
//!
//! Modal + HeaderStyle::Dialog chrome is canonical (per Agent O). This pass
//! fixes the BODY: kills the per-section `m = 10.0` margin literal in favor
//! of one body inner_margin derived from `gap_*()` tokens, hoists top-level
//! groups onto `PanelSection`, and uses `PanelSubSection` for the typography
//! sub-categories. FormRow / FormSection are kept for actual input rows.

use egui;
use super::super::style::*;
use crate::ui_kit::sx::Tone;
use super::super::super::gpu::{Watchlist, Theme, Chart};
use super::super::super::commands::{self, AppCommand};
use crate::ui_kit::widgets::{FormRow, FormRowAlign};
use super::super::components::text::{BodyLabel, SectionLabel};
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::{Variant, Size};
use crate::ui_kit::widgets::{ToggleRow, ThemePreviewCard, NumberStepper, PanelSection, PanelSubSection};
use crate::ui_kit::widgets::SegmentedControl;
use crate::ui_kit::widgets::theme_preview_card::PreviewKind;
use crate::ui_kit::widgets::modal::{Modal, Anchor, HeaderStyle, FrameKind};
use crate::chart_renderer::ui::foundation::text_style::TextStyle;

/// Unified body padding for the settings modal. Single source of truth —
/// every PanelSection / FormRow inside the scroll area inherits this and
/// nothing re-applies its own left indent. Picked `gap_md()` (12px) as the
/// closest token to the legacy `m = 10.0` literal so the visual delta is
/// imperceptible while gaining cross-section X alignment.
fn body_inset() -> f32 { gap_md() }

/// Shared chip metrics for the Appearance tab's four selector grids.
///
/// They had drifted to three heights (24 / 26 / 26) and two gutters (4 / 6),
/// with the vertical gutter differing from the horizontal in two of them —
/// four grids, same visual role, no two alike. One source now.
const CHIP_H: f32 = 26.0;
const CHIP_GAP: f32 = 6.0;

/// FormRow preset matching the legacy `srow()` look (190px label gutter,
/// muted left-aligned label, right-aligned body with 10px inner pad, xs
/// bottom margin) — but WITHOUT a per-row `leading_space`, because the
/// surrounding modal body already provides one consistent left inset.
/// Inlining this at every call site would obscure the actual content.
fn setting_form_row<'a>(label: &'a str, t: &Theme) -> FormRow<'a> {
    FormRow::new(label)
        .gutter(190.0)
        .label_left(true)
        .label_color(tint(t, Tone::Text, crate::ui_kit::style::alpha_near_solid()))
        .alignment(FormRowAlign::Right)
        .inner_pad(10.0)
        .margins(0.0, gap_xs())
}

/// Read-or-create a persistent collapse bool keyed by id_salt. Used by the
/// few PanelSubSection groupings inside Settings (TYPOGRAPHY → FONT FAMILY
/// / SIZE SCALE) so expanded state survives across modal open/close.
fn read_persisted_bool(ui: &egui::Ui, id_salt: &str, default: bool) -> bool {
    let id = ui.make_persistent_id(("settings_collapse", id_salt));
    ui.data_mut(|d| *d.get_persisted_mut_or(id, default))
}

fn write_persisted_bool(ui: &egui::Ui, id_salt: &str, value: bool) {
    let id = ui.make_persistent_id(("settings_collapse", id_salt));
    ui.data_mut(|d| d.insert_persisted(id, value));
}

/// Settings tab selector.
#[derive(Clone, Copy, PartialEq)]
enum SettingsTab { Appearance, Chart, Trading, Notifications, Shortcuts }

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, chart: &mut Chart, t: &Theme, ap: usize) {
if !watchlist.settings_open { return; }

let screen = ctx.screen_rect();
let dialog_w = 580.0_f32;
let dialog_h = (screen.height() * 0.82).min(780.0).max(400.0);
let dialog_pos = egui::pos2(screen.center().x - dialog_w / 2.0, screen.center().y - dialog_h / 2.0);
let modal_resp = Modal::new("SETTINGS")
    .id("settings_panel")
    .ctx(ctx)
    .theme(t)
    .size(egui::vec2(dialog_w, dialog_h))
    .anchor(Anchor::Window { pos: Some(dialog_pos) })
    .header_style(HeaderStyle::Dialog)
    .frame_kind(FrameKind::DialogWindow)
    .separator(false)
    .show(|ui| {

        // ── Tab bar — `ui_kit::widgets::Tabs` (replaces legacy TabBar). ──
        const TAB_VARIANTS: &[SettingsTab] = &[
            SettingsTab::Appearance,
            SettingsTab::Chart,
            SettingsTab::Trading,
            SettingsTab::Notifications,
            SettingsTab::Shortcuts,
        ];
        const TAB_LABELS: &[&str] = &["Appearance", "Chart", "Trading", "Notifications", "Shortcuts"];
        let tab_id = egui::Id::new("settings_active_tab");
        let mut tab: SettingsTab = ui.data_mut(|d| *d.get_temp_mut_or(tab_id, SettingsTab::Appearance));
        let mut idx = TAB_VARIANTS.iter().position(|v| *v == tab).unwrap_or(0);
        ui.horizontal(|ui| {
            ui.add_space(gap_lg());
            crate::ui_kit::widgets::Tabs::new(&mut idx, TAB_LABELS)
                .treatment(crate::ui_kit::widgets::tabs::TabTreatment::Card)
                .show(ui, t);
        });
        tab = TAB_VARIANTS[idx.min(TAB_VARIANTS.len() - 1)];
        ui.data_mut(|d| d.insert_temp(tab_id, tab));
        separator(ui, tint(t, Tone::Border, alpha_muted()));
        ui.add_space(gap_sm());

        // ── Tab content in a scroll area ──
        //
        // ONE body Frame with a single inner_margin = body_inset(). Every
        // PanelSection / FormRow inside starts at the same X coordinate.
        // No per-section `ui.horizontal { ui.add_space(m); ... }` indent.
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_width(dialog_w - 20.0);
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(body_inset() as i8, gap_sm() as i8))
                .show(ui, |ui| {
                    match tab {
                        SettingsTab::Appearance => draw_appearance(ui, watchlist, chart, t, ap),
                        SettingsTab::Chart      => draw_chart(ui, watchlist, chart, t),
                        SettingsTab::Trading    => draw_trading(ui, watchlist, t),
                        SettingsTab::Notifications => draw_notifications(ui, t),
                        SettingsTab::Shortcuts  => draw_shortcuts(ui, watchlist, t),
                    }
                });
        });
    });
    if modal_resp.closed { watchlist.update_sidebar_state(|s| s.settings_open = false); }
}

// ═══════════════════════════════════════════════════════════════
// APPEARANCE TAB
// ═══════════════════════════════════════════════════════════════
fn draw_appearance(ui: &mut egui::Ui, watchlist: &mut Watchlist, chart: &mut Chart, t: &Theme, ap: usize) {
    // ── DESIGN SYSTEM — the named presets (DS-6.0 D2) ──
    //
    // The two axes below are the raw 22 x 9 matrix. Six of those pairings are
    // DESIGNED, and those six are exactly what 12-T5-CERTIFICATION vouches for;
    // the rest are reachable but were never drawn by anyone. So the presets go
    // first and set BOTH axes at once, and the raw pickers stay underneath for
    // people who want them.
    //
    // Resolution is by NAME, not by the index pair the capture harness uses —
    // installing a theme pack shifts every index, and a preset pinned to a
    // position would then quietly point at a different design.
    PanelSection::new("DESIGN SYSTEM").show(ui, t, |ui, t| {
        use crate::design_system::presets;
        let scheme_names: Vec<String> = crate::chart_renderer::gpu::get_all_themes()
            .iter().map(|th| th.name.to_string()).collect();
        let style_names: Vec<String> = crate::chart_renderer::ui::style::list_style_presets()
            .into_iter().map(|(_, n)| n).collect();
        let active = presets::active(&scheme_names, &style_names, chart.theme_idx, watchlist.style_idx as usize);

        ui.spacing_mut().item_spacing = egui::vec2(CHIP_GAP, CHIP_GAP);
        ui.horizontal_wrapped(|ui| {
            for p in presets::PRESETS {
                // A preset that cannot resolve EXACTLY is skipped rather than
                // shown pointing somewhere approximate.
                let Some((ti, si)) = presets::resolve(p, &scheme_names, &style_names) else { continue };
                let is_active = active.map(|a| a.id) == Some(p.id);
                let resp = Button::toggle(p.name, is_active)
                    .min_size(egui::vec2(96.0, CHIP_H))
                    .show(ui, t);
                crate::ui_kit::widgets::Tooltip::new(p.blurb).show(ui, &resp, t);
                if resp.clicked() {
                    // Both axes, one click — that is what makes it a preset.
                    commands::push(AppCommand::SetThemeIdx { pane: ap, idx: ti });
                    commands::push(AppCommand::SetStyleIdx { idx: si });
                }
                #[cfg(debug_assertions)]
                crate::dev_inspector::record(
                    crate::dev_inspector::WidgetRecord::from_response(
                        &format!("settings.preset.{}", p.id), "button", p.name, &resp, ui,
                    )
                );
            }
        });
        if active.is_none() {
            ui.add_space(gap_xs());
            // A hand-built pairing is legitimate, not an error state — say so
            // plainly rather than snapping the selection to the nearest preset.
            ui.label(
                egui::RichText::new("Custom pairing — not one of the certified presets")
                    .size(font_xs())
                    .color(t.dim),
            );
        }

        // ── Layout archetype (DS-6.0 D1) ──
        //
        // The theme supplies the default; this row is the per-WORKSPACE
        // override. "Theme default" is first and is the normal state — it
        // clears the override rather than pinning the theme's current value,
        // so switching design system keeps changing the layout with it.
        use crate::design_system::style_system::Archetype;
        ui.add_space(gap_sm());
        // Label on its own line, matching every other group in this panel.
        ui.label(
            egui::RichText::new("LAYOUT").size(font_xs()).color(t.dim),
        );
        ui.add_space(gap_xs());
        let current_override = watchlist.workspace_archetype();
        let theme_default = crate::chart_renderer::ui::style::active_style_system()
            .shell.archetype;
        let mut set_to: Option<Option<Archetype>> = None;
        // Match the preset grid above: same chip height, same gutter on BOTH
        // axes. These were 24 tall with a 6/8 gutter next to 26-tall chips on
        // a 6/6 gutter — same visual role, three different numbers.
        ui.spacing_mut().item_spacing = egui::vec2(CHIP_GAP, CHIP_GAP);
        ui.horizontal_wrapped(|ui| {
            let follow = Button::toggle("Theme default", current_override.is_none())
                .min_size(egui::vec2(110.0, CHIP_H))
                .show(ui, t);
            crate::ui_kit::widgets::Tooltip::new(
                // Name what following actually gets you, so "Theme default"
                // is not an opaque choice.
                format!("Follow the design system ({:?} right now)", theme_default),
            ).show(ui, &follow, t);
            if follow.clicked() { set_to = Some(None); }
            // Instrumented like its siblings. Without this the widget tree
            // showed the archetype row starting 116px right of every other row
            // in the panel — which reads as an indent bug but is just this
            // chip being invisible to the capture. An un-instrumented widget
            // does not merely go unmeasured; it makes its NEIGHBOURS look wrong.
            #[cfg(debug_assertions)]
            crate::dev_inspector::record(
                crate::dev_inspector::WidgetRecord::from_response(
                    "settings.archetype.follow", "button", "Theme default", &follow, ui,
                )
            );

            for a in Archetype::ALL {
                let on = current_override == Some(a);
                let resp = Button::toggle(a.name(), on)
                    .min_size(egui::vec2(104.0, CHIP_H))
                    .show(ui, t);
                if resp.clicked() {
                    // Clicking the active override again releases it — the
                    // alternative is a one-way door back to "Theme default".
                    set_to = Some(if on { None } else { Some(a) });
                }
                #[cfg(debug_assertions)]
                crate::dev_inspector::record(
                    crate::dev_inspector::WidgetRecord::from_response(
                        &format!("settings.archetype.{}", a.name()), "button", a.name(), &resp, ui,
                    )
                );
            }
        });
        if let Some(v) = set_to { watchlist.set_workspace_archetype(v); }
    });

    // ── THEME — big preview blocks with mini chart layout ──
    PanelSection::new("THEME").show(ui, t, |ui, t| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        ui.horizontal_wrapped(|ui| {
            let all_themes = crate::chart_renderer::gpu::get_all_themes();
            for (i, preview_theme) in all_themes.iter().enumerate() {
                let resp = ThemePreviewCard::new(preview_theme.name, preview_theme)
                    .selected(chart.theme_idx == i)
                    .preview_kind(PreviewKind::Chart)
                    .size(egui::vec2(86.0, 52.0))
                    .show(ui, t);
                if resp.clicked() {
                    commands::push(AppCommand::SetThemeIdx { pane: ap, idx: i });
                }
                #[cfg(debug_assertions)]
                crate::dev_inspector::record(
                    crate::dev_inspector::WidgetRecord::from_response(
                        &format!("settings.theme_card.{i}"), "button", preview_theme.name,
                        &resp, ui,
                    )
                );
            }
        });
    });

    // ── STYLE preset (Aperture / Octave / Meridien / …) ──
    PanelSection::new("STYLE").show(ui, t, |ui, t| {
        let presets = crate::chart_renderer::ui::style::list_style_presets();
        let cur_si = watchlist.style_idx.min(presets.len().saturating_sub(1));
        let btn_w: f32 = 78.0;
        let btn_h: f32 = CHIP_H;
        let row_w = ui.available_width().max(btn_w);
        let per_row = ((row_w + gap_xs()) / (btn_w + gap_xs())).floor().max(1.0) as usize;
        for chunk in presets.chunks(per_row) {
            ui.horizontal(|ui| {
                // Was gap_xs() (4.0) where the three neighbouring chip grids
                // all use 6.0 — a "6, 6, 6, 4" rhythm break in one panel.
                ui.spacing_mut().item_spacing.x = CHIP_GAP;
                for (id, name) in chunk {
                    let id_us = *id as usize;
                    let active = id_us == cur_si;
                    let style_resp = Button::toggle(name.as_str(), active)
                        .min_size(egui::vec2(btn_w, btn_h))
                        .show(ui, t);
                    if style_resp.clicked() {
                        commands::push(AppCommand::SetStyleIdx { idx: id_us });
                    }
                    #[cfg(debug_assertions)]
                    crate::dev_inspector::record(
                        crate::dev_inspector::WidgetRecord::from_response(
                            &format!("settings.style_btn.{id_us}"), "button", name.as_str(),
                            &style_resp, ui,
                        )
                    );
                }
            });
            ui.add_space(gap_xs());
        }
    });

    // AUDIT 2026-08: the "COMPONENTS · Sx preview" gallery lived here. It was
    // the ONLY consumer of `ui_kit::sx::recipes` — a second, parallel recipe
    // system (a cva-style layer) sitting alongside the live `RecipeSpec` one.
    //
    // Its own comment called it "proof the new styling system is wired into the
    // app", but the system it proved had exactly one call site: this gallery.
    // The recipe layer that actually paints widgets is `RecipeSpec`, with 34
    // consumption points. Two recipe systems is one too many, so the cva layer
    // and its demo are deleted rather than left to look load-bearing.

    // ── DENSITY override (Compact / Standard / Spacious) — P4.3 ──
    // Independent of palette and style preset; overrides StyleSettings.density.
    // None = inherit from active style preset.
    PanelSection::new("DENSITY").show(ui, t, |ui, t| {
        use crate::ui_kit::style::DensityMode;
        let cur_override = watchlist.density_override;
        let btn_w: f32 = 80.0;
        let btn_h: f32 = 26.0;
        // push_id scopes every Button::toggle Id inside this picker so the
        // "Inherit" / "Standard" labels can't collide with the same labels in
        // the BORDER / CORNER / SPACING / MOTION pickers (egui hashes the
        // label as part of the Id; without a scope they all hash equally and
        // egui panics with "Widget N changed layer_id during the frame").
        ui.push_id("density_picker", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap_xs();
                let inherit_active = cur_override.is_none();
                if Button::toggle("Inherit", inherit_active)
                    .min_size(egui::vec2(btn_w, btn_h))
                    .show(ui, t).clicked() {
                    commands::push(AppCommand::SetDensityOverride(None));
                }
                for &mode in DensityMode::all() {
                    let active = cur_override == Some(mode);
                    if Button::toggle(mode.label(), active)
                        .min_size(egui::vec2(btn_w, btn_h))
                        .show(ui, t).clicked() {
                        commands::push(AppCommand::SetDensityOverride(Some(mode)));
                    }
                }
            });
        });
    });

    // ── BORDER WEIGHT (P5) — Hairline / Standard / Bold ──
    // Multiplier on every `stroke_*()` token. Hairline halves border thickness
    // across the entire app (Meridien-style minimal chrome); Bold thickens.
    PanelSection::new("BORDER WEIGHT").show(ui, t, |ui, t| {
        use crate::ui_kit::style::BorderWeight;
        let cur = watchlist.border_weight_override;
        let btn_w: f32 = 80.0;
        let btn_h: f32 = 26.0;
        ui.push_id("border_weight_picker", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap_xs();
                let inherit_active = cur.is_none();
                if Button::toggle("Inherit", inherit_active)
                    .min_size(egui::vec2(btn_w, btn_h))
                    .show(ui, t).clicked() {
                    watchlist.border_weight_override = None;
                    crate::ui_kit::style::set_border_weight_override(None);
                }
                for &mode in BorderWeight::all() {
                    let active = cur == Some(mode);
                    if Button::toggle(mode.label(), active)
                        .min_size(egui::vec2(btn_w, btn_h))
                        .show(ui, t).clicked() {
                        watchlist.border_weight_override = Some(mode);
                        crate::ui_kit::style::set_border_weight_override(Some(mode));
                    }
                }
            });
        });
    });

    // ── CORNER RADIUS (P5) — Sharp / Subtle / Standard / Round ──
    // Multiplier on every `radius_*()` token. Sharp = 0× → zero rounding across
    // the whole app (the Meridien square-corner aesthetic, but as a user knob).
    PanelSection::new("CORNER RADIUS").show(ui, t, |ui, t| {
        use crate::ui_kit::style::CornerScale;
        let cur = watchlist.corner_scale_override;
        let btn_w: f32 = 72.0;
        let btn_h: f32 = 26.0;
        ui.push_id("corner_scale_picker", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap_xs();
                let inherit_active = cur.is_none();
                if Button::toggle("Inherit", inherit_active)
                    .min_size(egui::vec2(btn_w, btn_h))
                    .show(ui, t).clicked() {
                    watchlist.corner_scale_override = None;
                    crate::ui_kit::style::set_corner_scale_override(None);
                }
                for &mode in CornerScale::all() {
                    let active = cur == Some(mode);
                    if Button::toggle(mode.label(), active)
                        .min_size(egui::vec2(btn_w, btn_h))
                        .show(ui, t).clicked() {
                        watchlist.corner_scale_override = Some(mode);
                        crate::ui_kit::style::set_corner_scale_override(Some(mode));
                    }
                }
            });
        });
    });

    // ── SPACING SCALE (P5) — Tight / Standard / Loose ──
    // Multiplier on every `gap_*()` token. Independent from DENSITY which
    // scales heights; SPACING SCALE controls horizontal/vertical gutters.
    PanelSection::new("SPACING SCALE").show(ui, t, |ui, t| {
        use crate::ui_kit::style::SpacingScale;
        let cur = watchlist.spacing_scale_override;
        let btn_w: f32 = 80.0;
        let btn_h: f32 = 26.0;
        ui.push_id("spacing_scale_picker", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap_xs();
                let inherit_active = cur.is_none();
                if Button::toggle("Inherit", inherit_active)
                    .min_size(egui::vec2(btn_w, btn_h))
                    .show(ui, t).clicked() {
                    watchlist.spacing_scale_override = None;
                    crate::ui_kit::style::set_spacing_scale_override(None);
                }
                for &mode in SpacingScale::all() {
                    let active = cur == Some(mode);
                    if Button::toggle(mode.label(), active)
                        .min_size(egui::vec2(btn_w, btn_h))
                        .show(ui, t).clicked() {
                        watchlist.spacing_scale_override = Some(mode);
                        crate::ui_kit::style::set_spacing_scale_override(Some(mode));
                    }
                }
            });
        });
    });

    // ── MOTION SPEED (P5) — Off / Fast / Standard / Slow ──
    // Multiplier on every `motion_*()` duration. Off disables animations
    // entirely — useful for accessibility, RDP latency, or distraction-free
    // focus during fast trading.
    PanelSection::new("MOTION SPEED").show(ui, t, |ui, t| {
        use crate::ui_kit::style::MotionSpeed;
        let cur = watchlist.motion_speed_override;
        let btn_w: f32 = 72.0;
        let btn_h: f32 = 26.0;
        ui.push_id("motion_speed_picker", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap_xs();
                let inherit_active = cur.is_none();
                if Button::toggle("Inherit", inherit_active)
                    .min_size(egui::vec2(btn_w, btn_h))
                    .show(ui, t).clicked() {
                    watchlist.motion_speed_override = None;
                    crate::ui_kit::style::set_motion_speed_override(None);
                }
                for &mode in MotionSpeed::all() {
                    let active = cur == Some(mode);
                    if Button::toggle(mode.label(), active)
                        .min_size(egui::vec2(btn_w, btn_h))
                        .show(ui, t).clicked() {
                        watchlist.motion_speed_override = Some(mode);
                        crate::ui_kit::style::set_motion_speed_override(Some(mode));
                    }
                }
            });
        });
    });

    // ── TYPOGRAPHY — collapsible sub-categories ──
    PanelSection::new("TYPOGRAPHY").show(ui, t, |ui, t| {
        // SIZE SCALE
        let mut size_open = read_persisted_bool(ui, "typo_size", true);
        let prev_size = size_open;
        PanelSubSection::new("typo_size", "SIZE SCALE")
            .expanded(&mut size_open)
            .show(ui, t, |ui, t| {
                setting_form_row("Size", t).show(ui, t, |ui| {
                    let display_pct = ((watchlist.font_scale - 0.96) / 0.016).round() as i32 + 60;
                    let mut dp = display_pct.clamp(60, 160);
                    if crate::ui_kit::widgets::Slider::new(&mut dp, 60..=160)
                        .step(1.0)
                        .show_value(true)
                        .show(ui, t)
                        .changed() {
                        watchlist.font_scale = 0.96 + (dp - 60) as f32 * 0.016;
                    }
                });
                ui.horizontal(|ui| {
                    for (label, ppp) in [(60, 0.96_f32), (80, 1.28), (100, 1.6), (120, 1.92), (140, 2.24), (160, 2.56)] {
                        let active = (watchlist.font_scale - ppp).abs() < 0.05;
                        let pct_label = format!("{}%", label);
                        if Button::toggle(pct_label.as_str(), active)
                            .min_size(egui::vec2(34.0, row_height_compact()))
                            .show(ui, t).clicked() {
                            watchlist.font_scale = ppp;
                        }
                    }
                });
            });
        if size_open != prev_size { write_persisted_bool(ui, "typo_size", size_open); }

        // FONT FAMILY
        let mut family_open = read_persisted_bool(ui, "typo_family", true);
        let prev_family = family_open;
        PanelSubSection::new("typo_family", "FONT FAMILY")
            .expanded(&mut family_open)
            .show(ui, t, |ui, t| {
                let font_names = crate::ui_kit::icons::FONT_NAMES;
                let current_idx = watchlist.font_idx.min(font_names.len() - 1);
                let card_w = 160.0;
                let card_h = 46.0;
                let cols = 3;
                let is_mono = [true, true, true, false, false, false];

                for row_start in (0..font_names.len()).step_by(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                        for i in row_start..(row_start + cols).min(font_names.len()) {
                            let name = font_names[i];
                            let sel = current_idx == i;
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                            crate::chart_renderer::ui::style::cursor::clickable(ui, &resp);

                            let bg = if sel { tint(t, Tone::Accent, alpha_tint()) }
                                else if resp.hovered() { tint(t, Tone::Border, alpha_subtle()) }
                                else { tint(t, Tone::Border, alpha_faint()) };
                            let border_col = if sel { t.accent }
                                else if resp.hovered() { tint(t, Tone::Accent, alpha_line()) }
                                else { tint(t, Tone::Border, alpha_muted()) };
                            ui.painter().rect_filled(r, radius_md(), bg);
                            ui.painter().rect_stroke(r, radius_md(),
                                egui::Stroke::new(if sel { stroke_bold() } else { stroke_thin() }, border_col), egui::StrokeKind::Outside);

                            let name_col = if sel { t.accent } else { t.text };
                            ui.painter().text(
                                egui::pos2(r.center().x, r.top() + 14.0),
                                egui::Align2::CENTER_CENTER,
                                name, mono_sm(), name_col);

                            let type_label = if is_mono[i.min(is_mono.len()-1)] { "mono" } else { "sans" };
                            let type_col = color_dim(t.dim);
                            ui.painter().text(
                                egui::pos2(r.left() + crate::ui_kit::style::gap_sm(), r.bottom() - crate::ui_kit::style::gap_md()),
                                egui::Align2::LEFT_CENTER,
                                type_label, mono_sm(), type_col);

                            let sample_col = if sel { t.text } else { color_subtle(t.dim) };
                            ui.painter().text(
                                egui::pos2(r.right() - crate::ui_kit::style::gap_sm(), r.bottom() - crate::ui_kit::style::gap_md()),
                                egui::Align2::RIGHT_CENTER,
                                "0123 AAPL $9.50", mono_xs(), sample_col);

                            // Focus ring for keyboard navigation.
                            cursor::focus_ring(ui, &resp, t.accent);
                            if resp.clicked() && !sel {
                                watchlist.font_idx = i;
                                crate::ui_kit::icons::init_fonts(ui.ctx(), i);
                            }
                        }
                    });
                }
            });
        if family_open != prev_family { write_persisted_bool(ui, "typo_family", family_open); }
    });

    // ── LAYOUT ──
    PanelSection::new("LAYOUT").show(ui, t, |ui, t| {
        setting_toggle_described(ui, "Compact Toolbar",
            Some("Trade vertical density for visual breathing room."),
            t, &mut watchlist.compact_mode);
        setting_toggle_described(ui, "Auto-Hide Toolbar",
            Some("Hide the toolbar until the cursor approaches the top of the pane."),
            t, &mut watchlist.toolbar_auto_hide);
        setting_form_row("Pane Headers", t).show(ui, t, |ui| {
            use crate::chart_renderer::PaneHeaderSize;
            let current = watchlist.pane_header_size;
            let labels = [
                (PaneHeaderSize::Compact, "Compact"),
                (PaneHeaderSize::Normal, "Normal"),
                (PaneHeaderSize::Expanded, "Expanded"),
            ];
            let mut active_idx = labels.iter().position(|(s, _)| *s == current).unwrap_or(0);
            const PANE_HEADER_OPTS: &[(usize, &str)] = &[(0, "Compact"), (1, "Normal"), (2, "Expanded")];
            if SegmentedControl::new(&mut active_idx, PANE_HEADER_OPTS).show(ui, t).changed() {
                watchlist.pane_header_size = labels[active_idx.min(labels.len() - 1)].0;
            }
        });
    });

    // ── THEMES — ThemePack install / activate / uninstall ──────────────────
    draw_themes_section(ui, chart, t, ap);

    // ── THEME STUDIO — full-screen theme authoring overlay ─────────────────
    PanelSection::new("THEME STUDIO").show(ui, t, |ui, t| {
        ui.horizontal(|ui| {
            ui.label(
                TextStyle::Caption.as_rich_cascading("Visual component storybook and live theme editor.", t.dim),
            );
        });
        ui.add_space(gap_xs());
        if Button::new("Open Theme Studio")
            .variant(crate::ui_kit::widgets::tokens::Variant::Secondary)
            .size(crate::ui_kit::widgets::tokens::Size::Sm)
            .show(ui, t)
            .clicked()
        {
            crate::chart_renderer::ui::theme_studio::open(ui.ctx());
            // Close settings panel so studio fills the screen without overlap.
            watchlist.update_sidebar_state(|s| s.settings_open = false);
        }
    });

    // ── ONBOARDING ──
    PanelSection::new("ONBOARDING").show(ui, t, |ui, t| {
        setting_form_row("Welcome wizard", t).show(ui, t, |ui| {
            if Button::new("Show welcome again")
                .variant(crate::ui_kit::widgets::tokens::Variant::Secondary)
                .size(crate::ui_kit::widgets::tokens::Size::Sm)
                .show(ui, t)
                .clicked()
            {
                watchlist.update_ui_settings(|s| {
                    s.has_seen_welcome = false;
                    s.welcome_step_resume = 0;
                });
                watchlist.welcome_wizard = Some(
                    crate::chart_renderer::ui::welcome::WelcomeWizard::from_settings(false, 0)
                );
            }
        });
    });
}

// ═══════════════════════════════════════════════════════════════
// CHART TAB
// ═══════════════════════════════════════════════════════════════
fn draw_chart(ui: &mut egui::Ui, watchlist: &mut Watchlist, chart: &mut Chart, t: &Theme) {
    // ── AXES & GRID ──
    PanelSection::new("AXES & GRID").show(ui, t, |ui, t| {
        setting_toggle_described(ui, "Show X-Axis (time)",
            Some("Render the time axis along the bottom of every chart pane."),
            t, &mut watchlist.show_x_axis);
        setting_toggle_described(ui, "Show Y-Axis (price)",
            Some("Render the price axis along the right edge of every chart pane."),
            t, &mut watchlist.show_y_axis);
        setting_toggle_described(ui, "Shared X-Axis (multi-pane)",
            Some("Stacked panes scroll and zoom together on the time axis."),
            t, &mut watchlist.shared_x_axis);
        setting_toggle_described(ui, "Shared Y-Axis (multi-pane)",
            Some("Stacked panes share a synchronized price scale."),
            t, &mut watchlist.shared_y_axis);
    });

    // ── CHART BEHAVIOR ──
    PanelSection::new("CHART BEHAVIOR").show(ui, t, |ui, t| {
        setting_toggle_described(ui, "OHLC Tooltip",
            Some("Show open/high/low/close values for the bar under the cursor."),
            t, &mut chart.ohlc_tooltip);
        setting_toggle_described(ui, "Magnet Snap",
            Some("Snap drawings and the crosshair to the nearest OHLC level."),
            t, &mut chart.magnet);
        setting_toggle_described(ui, "Log Scale",
            Some("Use a logarithmic price axis so equal % moves render at equal heights."),
            t, &mut chart.log_scale);
        setting_toggle(ui, "Show Volume", t, &mut chart.show_volume);
        setting_toggle(ui, "Show Oscillators", t, &mut chart.show_oscillators);
    });

    // ── SESSIONS ──
    // Wave 9c: prefer registry-backed truth via `symbol_meta` so a real
    // equity ticker that happens to end in USDT doesn't get the "N/A for
    // crypto" 24/7 treatment.
    let is_crypto = chart.symbol_meta.is_crypto();
    PanelSection::new("SESSIONS").show(ui, t, |ui, t| {
        if is_crypto {
            ui.add(BodyLabel::new("N/A for crypto (24/7 market)").color(color_half(t.dim)));
        } else {
            setting_toggle_described(ui, "Session Shading",
                Some("Visually distinguish regular and extended trading hours on the chart."),
                t, &mut chart.session_shading);
            if chart.session_shading {
                setting_form_row("ETH Bar Opacity", t).show(ui, t, |ui| {
                    let mut pct = (chart.eth_bar_opacity * 100.0).round() as i32;
                    if crate::ui_kit::widgets::Slider::new(&mut pct, 0..=100)
                        .step(1.0)
                        .show_value(true)
                        .show(ui, t)
                        .changed() {
                        chart.eth_bar_opacity = (pct as f32 / 100.0).clamp(0.0, 1.0);
                    }
                });
                setting_toggle(ui, "Background Tint", t, &mut chart.session_bg_tint);
                if chart.session_bg_tint {
                    ui.horizontal(|ui| {
                        ui.add_space(gap_sm());
                        for (label, hex) in [("Navy", "#1a1a2e"), ("Purple", "#2d1b4e"), ("Green", "#1a2e1a"), ("Red", "#2e1a1a"), ("Blue", "#1a2e3e")] {
                            let active = chart.session_bg_color == hex;
                            let c = hex_to_color(hex, 1.0);
                            let fg = if active { t.accent } else { tint(t, Tone::Text, crate::ui_kit::style::alpha_heavy()) };
                            let bg = if active { color_alpha(c, alpha_strong()) } else { color_alpha(c, alpha_muted()) };
                            if Button::new(label).variant(Variant::Chrome).size(Size::Xs).fg(fg)
                                .fill(bg).corner_radius(crate::ui_kit::style::radius_sm()).min_size(egui::vec2(38.0, row_height_dense())).show(ui, t).clicked() {
                                chart.session_bg_color = hex.to_string();
                            }
                        }
                    });
                    setting_form_row("Tint Opacity", t).show(ui, t, |ui| {
                        let mut pct = (chart.session_bg_opacity * 100.0).round() as i32;
                        if crate::ui_kit::widgets::Slider::new(&mut pct, 0..=100)
                            .step(1.0)
                            .show_value(true)
                            .show(ui, t)
                            .changed() {
                            chart.session_bg_opacity = (pct as f32 / 100.0).clamp(0.0, 1.0);
                        }
                    });
                }
                setting_toggle(ui, "Session Break Lines", t, &mut chart.session_break_lines);
                let (sh, sm2, eh, em2) = (chart.rth_start_minutes / 60, chart.rth_start_minutes % 60,
                    chart.rth_end_minutes / 60, chart.rth_end_minutes % 60);
                ui.add(SectionLabel::new(&format!("RTH: {:02}:{:02} – {:02}:{:02} ET", sh, sm2, eh, em2))
                    .tiny().color(color_dim(t.dim)));
            }
        }
    });
}

// ═══════════════════════════════════════════════════════════════
// NOTIFICATIONS TAB
// ═══════════════════════════════════════════════════════════════
fn draw_notifications(ui: &mut egui::Ui, t: &Theme) {
    // Master toggles + per-category routing (toolbar ticker vs toasts).
    PanelSection::new("NOTIFICATIONS").show(ui, t, |ui, t| {
        use crate::chart_renderer::ui::tools::notification as notif;
        let mut p = notif::routing_from_ctx(ui.ctx());
        let before = (p.toolbar_enabled, p.toasts_enabled,
            p.orders.to_u8(), p.signals.to_u8(), p.alerts.to_u8(), p.system.to_u8());

        setting_toggle_described(ui, "Toolbar ticker",
            Some("Show notifications in the toolbar ticker strip."), t, &mut p.toolbar_enabled);
        setting_toggle_described(ui, "Toasts (bottom-left)",
            Some("Show pop-up toasts above the footer."), t, &mut p.toasts_enabled);

        ui.add_space(gap_xs());
        ui.add(SectionLabel::new("ROUTE BY TYPE").tiny().color(tint(t, Tone::Dim, crate::ui_kit::style::alpha_solid())));

        const DEST: &[(usize, &str)] = &[(0, "Off"), (1, "Toolbar"), (2, "Toast"), (3, "Both")];
        let route_row = |ui: &mut egui::Ui, label: &str, d: &mut notif::NotifDest| {
            setting_form_row(label, t).show(ui, t, |ui| {
                let mut v = d.to_u8() as usize;
                SegmentedControl::new(&mut v, DEST).show(ui, t);
                *d = notif::NotifDest::from_u8(v as u8);
            });
        };
        route_row(ui, "Orders", &mut p.orders);
        route_row(ui, "Signals", &mut p.signals);
        route_row(ui, "Price alerts", &mut p.alerts);
        route_row(ui, "System / Connection", &mut p.system);

        let after = (p.toolbar_enabled, p.toasts_enabled,
            p.orders.to_u8(), p.signals.to_u8(), p.alerts.to_u8(), p.system.to_u8());
        if after != before {
            notif::routing_to_ctx(ui.ctx(), p);
            notif::set_routing(p);
        }
    });
}

// ═══════════════════════════════════════════════════════════════
// TRADING TAB
// ═══════════════════════════════════════════════════════════════

/// W0-11 (audit): transient "the trader clicked toward LIVE and must confirm"
/// flag. A module static (not a persisted Watchlist field) because it is pure
/// per-session UI state on the single UI thread — it must never survive a
/// restart as an armed live switch.
static GO_LIVE_ARMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn draw_trading(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    // ── MODE (Paper / Live) ──
    PanelSection::new("MODE").show(ui, t, |ui, t| {
        use crate::chart_renderer::trading::order_manager as om;
        let was_paper = om::is_paper_mode();
        let mut paper = was_paper;
        setting_toggle_described(ui, "Paper Trading",
            Some("Route orders to the simulated account instead of the live broker."),
            t, &mut paper);
        if paper != was_paper {
            if paper {
                // W0-11: live → paper is risk-REDUCING — apply immediately.
                if let Err(reason) = om::set_paper_mode(true) {
                    crate::data::connectivity::errors_sink::report(
                        crate::data::connectivity::errors_sink::ErrorLevel::Warn,
                        "trading", "paper_mode_toggle_blocked", reason);
                }
                GO_LIVE_ARMED.store(false, std::sync::atomic::Ordering::Relaxed);
            } else {
                // W0-11 (audit): paper → LIVE was a single unconfirmed click.
                // Do NOT apply here — arm a confirmation instead. The toggle
                // re-reads the actual mode below, so it springs back to Paper
                // until the trader explicitly confirms.
                GO_LIVE_ARMED.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }

        // W0-11: explicit confirmation gate for going LIVE.
        if GO_LIVE_ARMED.load(std::sync::atomic::Ordering::Relaxed) && om::is_paper_mode() {
            ui.add_space(gap_xs());
            ui.add(SectionLabel::new(
                "\u{26A0} Switch to LIVE? Real orders will hit your broker.")
                .tiny().color(t.bear));
            let blocked = om::is_trading_blocked();
            if blocked {
                ui.add(SectionLabel::new(
                    "Kill switch / halt is engaged — release it before going live.")
                    .tiny().color(t.warn));
            }
            ui.horizontal(|ui| {
                // Confirm is disabled while trading is blocked.
                let confirm = Button::new("Go LIVE")
                    .variant(Variant::Primary).tint(t.bear).size(Size::Sm);
                let confirm = if blocked { confirm.enabled(false) } else { confirm };
                if confirm.show(ui, t).clicked() && !blocked {
                    if let Err(reason) = om::set_paper_mode(false) {
                        crate::data::connectivity::errors_sink::report(
                            crate::data::connectivity::errors_sink::ErrorLevel::Warn,
                            "trading", "go_live_blocked", reason);
                    }
                    GO_LIVE_ARMED.store(false, std::sync::atomic::Ordering::Relaxed);
                }
                if Button::new("Cancel").variant(Variant::Ghost).size(Size::Sm)
                    .show(ui, t).clicked()
                {
                    GO_LIVE_ARMED.store(false, std::sync::atomic::Ordering::Relaxed);
                }
            });
        }

        let paper = om::is_paper_mode();
        let (label, color) = if paper {
            ("Paper mode — orders go to simulated account", t.bull)
        } else {
            ("LIVE mode — real money at risk", t.bear)
        };
        ui.add(SectionLabel::new(label).tiny().color(color));
    });

    // ── ORDER DEFAULTS ──
    // Wave 2 (state): flat fields are mutated in-place for SegmentedControl
    // compatibility; push_to_trading_defaults_store() propagates every change
    // into the Store<TradingDefaults> so the persist supervisor can write.
    let mut trading_changed = false;
    PanelSection::new("ORDER DEFAULTS").show(ui, t, |ui, t| {
        setting_form_row("Stock Qty", t).show(ui, t, |ui| {
            let mut v = watchlist.default_stock_qty as i32;
            if NumberStepper::new(&mut v).range(1..=100_000).step(10.0).suffix(" shares").integer().show(ui, t).changed() {
                watchlist.default_stock_qty = v.max(1) as u32;
                trading_changed = true;
            }
        });
        setting_form_row("Options Qty", t).show(ui, t, |ui| {
            let mut v = watchlist.default_options_qty as i32;
            if NumberStepper::new(&mut v).range(1..=10_000).step(1.0).suffix(" contracts").integer().show(ui, t).changed() {
                watchlist.default_options_qty = v.max(1) as u32;
                trading_changed = true;
            }
        });
        setting_form_row("Order Type", t).show(ui, t, |ui| {
            const ORDER_TYPES: &[(usize, &str)] = &[(0, "MKT"), (1, "LMT"), (2, "STP")];
            let before = watchlist.default_order_type;
            SegmentedControl::new(&mut watchlist.default_order_type, ORDER_TYPES).show(ui, t);
            if watchlist.default_order_type != before { trading_changed = true; }
        });
        setting_form_row("Time in Force", t).show(ui, t, |ui| {
            const TIF_OPTS: &[(usize, &str)] = &[(0, "DAY"), (1, "GTC"), (2, "IOC")];
            let before = watchlist.default_tif;
            SegmentedControl::new(&mut watchlist.default_tif, TIF_OPTS).show(ui, t);
            if watchlist.default_tif != before { trading_changed = true; }
        });
        let before_rth = watchlist.default_outside_rth;
        setting_toggle(ui, "Outside RTH", t, &mut watchlist.default_outside_rth);
        if watchlist.default_outside_rth != before_rth { trading_changed = true; }
    });
    if trading_changed { watchlist.push_to_trading_defaults_store(); }

    // ── RISK MANAGEMENT ──
    PanelSection::new("RISK MANAGEMENT").show(ui, t, |ui, t| {
        use crate::chart_renderer::trading::order_manager;
        let mut limits = order_manager::get_risk_limits();
        setting_form_row("Max Order Qty", t).show(ui, t, |ui| {
            let mut v = limits.max_order_qty as i32;
            if NumberStepper::new(&mut v).range(1..=100_000).step(10.0).integer().show(ui, t).changed() {
                limits.max_order_qty = v.max(1) as u32;
            }
        });
        setting_form_row("Max Position", t).show(ui, t, |ui| {
            let mut v = limits.max_position_qty as i32;
            if NumberStepper::new(&mut v).range(1..=500_000).step(100.0).integer().show(ui, t).changed() {
                limits.max_position_qty = v.max(1) as u32;
            }
        });
        setting_form_row("Max Notional $", t).show(ui, t, |ui| {
            let mut v = limits.max_notional as i64;
            if ui.add(egui::DragValue::new(&mut v).range(0..=10_000_000).speed(1000)
                .custom_formatter(|v, _| if v as i64 == 0 { "OFF".into() } else { format!("${}", v as i64) })).changed() {
                limits.max_notional = v.max(0) as f64;
            }
        });
        setting_form_row("Fat Finger %", t).show(ui, t, |ui| {
            let mut v = limits.fat_finger_pct;
            if ui.add(egui::DragValue::new(&mut v).range(0.0..=50.0).speed(0.5).suffix("%")
                .custom_formatter(|v, _| if v < 0.1 { "OFF".into() } else { format!("{:.1}%", v) })).changed() {
                limits.fat_finger_pct = v.max(0.0);
            }
        });
        setting_form_row("Max Open Orders", t).show(ui, t, |ui| {
            let mut v = limits.max_open_orders as i32;
            if crate::ui_kit::widgets::Slider::new(&mut v, 1..=1000)
                .step(1.0)
                .show_value(true)
                .show(ui, t)
                .changed() {
                limits.max_open_orders = v.max(1) as usize;
            }
        });
        setting_form_row("Max Daily Loss $", t).show(ui, t, |ui| {
            let mut v = limits.max_daily_loss as i64;
            if ui.add(egui::DragValue::new(&mut v).range(0..=1_000_000).speed(500)
                .custom_formatter(|v, _| if v as i64 == 0 { "OFF".into() } else { format!("${}", v as i64) })).changed() {
                limits.max_daily_loss = v.max(0) as f64;
            }
        });
        setting_form_row("Dedup Cooldown", t).show(ui, t, |ui| {
            let mut v = limits.dedup_cooldown_ms as i32;
            if crate::ui_kit::widgets::Slider::new(&mut v, 100..=5000)
                .step(50.0)
                .show_value(true)
                .show(ui, t)
                .changed() {
                limits.dedup_cooldown_ms = v.max(100) as u64;
            }
        });
        order_manager::update_risk_limits(limits);
    });

    // ── APEX DATA ──
    PanelSection::new("APEX DATA").show(ui, t, |ui, t| {
        let mut enabled = crate::apex_data::is_enabled();
        let prev = enabled;
        setting_toggle_described(ui, "Enabled",
            Some("Stream live market data from the ApexData feed."),
            t, &mut enabled);
        if enabled != prev {
            crate::apex_data::set_enabled(enabled);
            if enabled { crate::apex_data::ws::start(); }
        }

        setting_form_row("Base URL", t)
            .show_with_cx(ui, t, |ui, _cx| {
                let id = egui::Id::new("apex_data_url_edit");
                let mut buf: String = ui.data_mut(|d|
                    d.get_temp::<String>(id).unwrap_or_else(|| crate::apex_data::apex_url()));
                let resp = crate::ui_kit::widgets::Input::new(&mut buf)
                    .min_width(340.0)
                    .show(ui, t);
                if resp.response.changed() { ui.data_mut(|d| d.insert_temp(id, buf.clone())); }
                if resp.submitted {
                    crate::apex_data::set_apex_url(buf.trim().to_string());
                }
            });

        setting_form_row("Auth Token", t)
            .password(true)
            .hint("optional — leave blank if no token required")
            .show_with_cx(ui, t, |ui, cx| {
                let id = egui::Id::new("apex_data_token_edit");
                let mut buf: String = ui.data_mut(|d|
                    d.get_temp::<String>(id).unwrap_or_else(|| crate::apex_data::apex_token().unwrap_or_default()));
                let mut input = crate::ui_kit::widgets::Input::new(&mut buf).min_width(340.0);
                if cx.password { input = input.password(true); }
                if let Some(h) = cx.hint { input = input.placeholder(h); }
                let resp = input.show(ui, t);
                if resp.response.changed() { ui.data_mut(|d| d.insert_temp(id, buf.clone())); }
                if resp.submitted {
                    let tok = buf.trim();
                    crate::apex_data::set_apex_token(if tok.is_empty() { None } else { Some(tok.to_string()) });
                }
            });

        let ws_connected = crate::apex_data::ws::is_connected();
        let (state_label, state_col) = if ws_connected {
            ("WS connected", t.bull)
        } else {
            ("WS disconnected", t.bear)
        };
        ui.add(SectionLabel::new(state_label).tiny().color(state_col));
    });
}

// ═══════════════════════════════════════════════════════════════
// SHORTCUTS TAB
// ═══════════════════════════════════════════════════════════════
fn draw_shortcuts(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    // Column header (uses the body inset, no per-row indent).
    ui.horizontal(|ui| {
        ui.allocate_ui(egui::vec2(220.0, 16.0), |ui| {
            ui.add(SectionLabel::new("ACTION").tiny().color(color_dim(t.dim)));
        });
        ui.add(SectionLabel::new("SHORTCUT").tiny().color(color_dim(t.dim)));
    });
    separator(ui, tint(t, Tone::Border, alpha_muted()));
    ui.add_space(gap_xs());

    super::super::tools::hotkey_editor::draw_content(ui, watchlist, t);
}

// ─── Helpers: standard setting toggles ─────────────────────────

fn setting_toggle(ui: &mut egui::Ui, label: &str, t: &Theme, val: &mut bool) {
    setting_toggle_described(ui, label, None, t, val);
}

// ═══════════════════════════════════════════════════════════════
// THEMES SECTION (inside Appearance tab)
// ═══════════════════════════════════════════════════════════════
//
// Lists all registered ThemePacks (built-in + installed) in a scrollable
// section. Each row shows:
//  - pack name + author (installed) or "Built-in" label
//  - an active indicator dot
//  - clicking the row activates the pack via `theme_pack_bridge::activate_by_id`
//  - an "×" trailing button uninstalls installed packs
//
// An "Install…" button at the bottom of the section opens an rfd file dialog
// for a `.apextheme` bundle, calls `install_pack`, and refreshes the list.
// Any install/validation error is shown in a one-line error message below.
//
// Persistence: `set_active` writes to `PackRegistry`'s `index.json`.
// On startup `init_registry` re-applies the persisted active pack.
//
// Light-theme note: all colours are derived from the threaded `t: &Theme`
// (accent, dim, bull, bear). No hardcoded RGB.

fn draw_themes_section(
    ui: &mut egui::Ui,
    chart: &mut Chart,
    t: &Theme,
    _ap: usize,
) {
    use crate::chart_renderer::theme_pack_bridge;
    use crate::ui_kit::widgets::{PanelListRow, TrailingBtn, TrailingTone, PanelTone};
    use crate::ui_kit::icons::Icon;

    // ── Persistent error string stored in egui memory ─────────────────────────
    let err_id  = ui.make_persistent_id("themes_install_err");
    let mut err = ui.data_mut(|d| d.get_temp::<String>(err_id).unwrap_or_default());

    // We need `action_clicked` from the section response, but the closure
    // captures `err` by mutable reference so we collect click events in a
    // `Cell` trampoline and process them after the section closes.
    let activate_id: std::cell::Cell<Option<String>> = std::cell::Cell::new(None);
    let uninstall_id: std::cell::Cell<Option<String>> = std::cell::Cell::new(None);

    let resp = PanelSection::new("THEMES")
        .action("Install…", PanelTone::Accent)
        .show(ui, t, |ui, t| {
            let packs      = theme_pack_bridge::list_pack_manifests();
            let active     = theme_pack_bridge::active_pack_id();
            let all_themes = crate::chart_renderer::gpu::get_all_themes();

            // ── List all packs ────────────────────────────────────────────────
            for pack in &packs {
                let is_active_pack = active.as_deref() == Some(pack.id.as_str());
                // Also consider "active" if this color-scheme name matches the
                // currently selected chart theme (covers built-in selection).
                let theme_name_match = all_themes
                    .get(chart.theme_idx)
                    .map(|th| th.name == pack.name.as_str())
                    .unwrap_or(false);
                let active_indicator = is_active_pack || theme_name_match;

                let author_str: String = if pack.author.is_empty() {
                    "Built-in".to_string()
                } else {
                    pack.author.clone()
                };
                let is_installed = !theme_pack_bridge::is_builtin(&pack.id);
                let pack_id = pack.id.clone();

                // ── Active dot closure ────────────────────────────────────────
                // Type is inferred as &Theme from the PanelSection::show generic.
                let dot_col = if active_indicator {
                    t.accent
                } else {
                    tint(t, crate::ui_kit::sx::Tone::Border, alpha_muted())
                };
                let dot = move |ui: &mut egui::Ui, _t: &Theme| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.5, dot_col);
                };

                let row_resp = if is_installed {
                    PanelListRow::new(pack_id.as_str())
                        .primary(pack.name.as_str())
                        .secondary(author_str.as_str())
                        .leading(dot)
                        .trailing_buttons(&[
                            TrailingBtn::icon(Icon::X)
                                .tone(TrailingTone::Bear)
                                .tooltip("Uninstall"),
                        ])
                        .show_full(ui, t)
                } else {
                    PanelListRow::new(pack_id.as_str())
                        .primary(pack.name.as_str())
                        .secondary("Built-in")
                        .leading(dot)
                        .show_full(ui, t)
                };

                // Row body click → activate (via Cell trampoline).
                if row_resp.clicked {
                    activate_id.set(Some(pack.id.clone()));
                }
                // Trailing X → uninstall.
                if row_resp.trailing_clicked_idx == Some(0) {
                    uninstall_id.set(Some(pack.id.clone()));
                }
            }

            // ── Error message (inside section) ────────────────────────────────
            if !err.is_empty() {
                ui.add_space(gap_xs());
                ui.label(
                    TextStyle::Caption.as_rich_cascading(err.as_str(), t.bear),
                );
            }
        });

    // ── Process deferred actions ──────────────────────────────────────────────
    if let Some(id) = activate_id.take() {
        let ctx = ui.ctx().clone();
        match theme_pack_bridge::activate_by_id(&ctx, &id) {
            Ok(_)  => { err.clear(); }
            Err(e) => { err = format!("Activate failed: {e}"); }
        }
        ui.data_mut(|d| d.insert_temp(err_id, err.clone()));
    }
    if let Some(id) = uninstall_id.take() {
        match theme_pack_bridge::uninstall_pack(&id) {
            Ok(_)  => { err.clear(); }
            Err(e) => { err = format!("Uninstall failed: {e}"); }
        }
        ui.data_mut(|d| d.insert_temp(err_id, err.clone()));
    }

    // ── "Install…" action button ──────────────────────────────────────────────
    if resp.action_clicked {
        // rfd::FileDialog is synchronous on Windows / macOS.
        let picked = rfd::FileDialog::new()
            .set_title("Install Theme Pack")
            .add_filter("Apex Theme Pack", &["apextheme"])
            .pick_file();

        if let Some(path) = picked {
            match theme_pack_bridge::install_pack(&path) {
                Ok(manifest) => {
                    let ctx = ui.ctx().clone();
                    match theme_pack_bridge::activate_by_id(&ctx, &manifest.id) {
                        Ok(_)  => { err.clear(); }
                        Err(e) => { err = format!("Installed but activate failed: {e}"); }
                    }
                }
                Err(e) => { err = format!("Install failed: {e}"); }
            }
            ui.data_mut(|d| d.insert_temp(err_id, err.clone()));
        }
    }
}

/// ToggleRow wrapper with optional description. No per-row indent: the
/// outer body Frame already provides the consistent left inset.
fn setting_toggle_described(
    ui: &mut egui::Ui,
    label: &str,
    description: Option<&str>,
    t: &Theme,
    val: &mut bool,
) {
    let mut row = ToggleRow::new(val).label(label);
    if let Some(desc) = description {
        row = row.description(desc);
    }
    row.show(ui, t);
    ui.add_space(gap_xs());
}
