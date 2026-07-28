//! Theme Studio — full-window overlay for visual QA + live theme authoring.
//!
//! ## Layout
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │  THEME STUDIO                                    [Apply] [X Close]  │
//! ├───────────────────────────────────────┬─────────────────────────────┤
//! │  CATALOG (left / scrollable)          │  EDITOR (right ~320px)      │
//! │  • Buttons (5 variants × 3 sizes)     │  Color pickers per tone     │
//! │  • Tabs                               │  Radii / stroke / spacing   │
//! │  • Tags / Badges                      │  Manifest fields             │
//! │  • PanelSection + PanelListRow        │  [Import] [Export]           │
//! │  • Tooltip, Input                     │  Validation report           │
//! └───────────────────────────────────────┴─────────────────────────────┘
//! ```
//!
//! ## State (egui memory, NOT Watchlist / Chart)
//!
//! | Key (Id::new) | Type | Purpose |
//! |---|---|---|
//! | `"theme_studio_open"` | `bool` | Open/close flag |
//! | `"theme_studio_pack"` | `WorkingPack` | Working ThemePack being edited |
//!
//! ## Live-preview push/restore
//!
//! While drawing the CATALOG region the working pack's ColorScheme is
//! converted to a `PortableTheme` and pushed into egui ambient
//! (`set_ambient_theme` + `set_ambient_recipes`). After the CATALOG the
//! original ambient is restored. This is the practical preview path.
//!
//! ## Core constraint
//!
//! `draw_theme_studio` is called from `top_nav::render` (via
//! `settings_panel::draw` insertion point) — NOT from `core.rs`.
//! No fields are added to `Watchlist` or `Chart`.

#![allow(clippy::too_many_arguments)]

use std::collections::BTreeMap;
use std::sync::Arc;
use egui::{Color32, Context, Id, ScrollArea, Ui};

use crate::design_system::import::{
    figma_export_to_pack, theme_to_figma_export, FigmaMapping, FigmaThemeExport, ImportOptions,
};
use crate::ui_kit::sx::recipe_spec::RecipeSpec;

use crate::{
    chart_renderer::{
        gpu::Theme,
        ui::style::{
            gap_lg, gap_md, gap_sm, gap_xs, radius_sm,
            stroke_thin,
        },
    },
    design_system::{
        color_scheme::{builtin_dark, ColorScheme, Rgba},
        recipes::RecipeSet,
        style_system::StyleSystem,
        theme_pack::{
            manifest::ThemeManifest,
            validate::{validate, AccessibilityMode},
            ThemePack,
        },
    },
    ui_kit::{
        assets::AssetRegistry,
        widgets::{
            theme::{
                set_ambient_recipes, set_ambient_theme, PortableTheme,
            },
            tokens::{Size, Variant},
            Badge, Button, Input, PanelListRow, PanelSection, Tag, TagTone, Tabs, Tooltip,
        },
    },
};

// ── egui memory keys ──────────────────────────────────────────────────────────

const KEY_OPEN:     &str = "theme_studio_open";
const KEY_PACK:     &str = "theme_studio_pack";
const KEY_CATALOG_SCROLL: &str = "theme_studio_catalog_tab";
const KEY_LIST_SEL: &str = "theme_studio_list_sel";
const KEY_INPUT_A:  &str = "theme_studio_input_a";

// ── WorkingPack — in-memory clone stored in egui memory ───────────────────────

/// In-memory working copy of the theme being authored. All fields are
/// `Clone + Send + Sync + 'static` so they fit in egui's `TypeMap`.
#[derive(Clone, Debug)]
pub struct WorkingPack {
    pub manifest:     ThemeManifest,
    pub color_scheme: ColorScheme,
    pub style_system: StyleSystem,
}

impl Default for WorkingPack {
    fn default() -> Self {
        Self {
            manifest:     {
                let mut m = ThemeManifest::new("my-theme", "My Theme");
                m.author  = String::new();
                m.version = "1.0.0".to_string();
                m
            },
            color_scheme: builtin_dark(),
            style_system: StyleSystem::builtin_default(),
        }
    }
}

// ── Public flag helpers ────────────────────────────────────────────────────────

/// Read whether the studio is currently open.
pub(crate) fn is_open(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(Id::new(KEY_OPEN))).unwrap_or(false)
}

/// Open the studio. If no working pack is stashed yet, seed from the current
/// active theme in egui ambient (or fall back to the builtin dark default).
pub(crate) fn open(ctx: &Context) {
    // Seed a fresh working pack if one isn't already stashed.
    let has_pack = ctx.data(|d| d.get_temp::<WorkingPack>(Id::new(KEY_PACK))).is_some();
    if !has_pack {
        let pack = WorkingPack::default();
        ctx.data_mut(|d| d.insert_temp(Id::new(KEY_PACK), pack));
    }
    ctx.data_mut(|d| d.insert_temp(Id::new(KEY_OPEN), true));
}

/// Close the studio without discarding the working pack.
pub(crate) fn close(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(Id::new(KEY_OPEN), false));
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Draw the full-window Theme Studio overlay when it is open.
///
/// Called from `top_nav::render` immediately after `settings_panel::draw`.
/// Draws as an `egui::Window` spanning the full screen (z-order above everything
/// except native menus).  No fields on `Watchlist` or `Chart` are touched.
pub(crate) fn draw_theme_studio(ctx: &Context, t: &Theme) {
    if !is_open(ctx) {
        return;
    }

    let screen = ctx.screen_rect();
    // Full-screen overlay window.
    egui::Window::new("##theme_studio_root")
        .id(Id::new("theme_studio_window"))
        .fixed_rect(screen)
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .frame(
            egui::Frame::new()
                .fill(t.bg)
                .stroke(egui::Stroke::NONE)
                .corner_radius(egui::CornerRadius::ZERO),
        )
        .show(ctx, |ui| {
            draw_studio_content(ui, ctx, t, screen.width());
        });
}

// ── Studio content ────────────────────────────────────────────────────────────

fn draw_studio_content(ui: &mut Ui, ctx: &Context, t: &Theme, total_w: f32) {
    // Load / mutate working pack via egui memory.
    let mut pack: WorkingPack = ctx
        .data(|d| d.get_temp::<WorkingPack>(Id::new(KEY_PACK)))
        .unwrap_or_default();

    // ── Title bar ─────────────────────────────────────────────────────────────
    let title_h = 44.0_f32;
    ui.set_min_size(egui::vec2(total_w, ui.available_height()));

    egui::Frame::new()
        .fill(t.toolbar_bg)
        .stroke(egui::Stroke::new(stroke_thin(), t.toolbar_border))
        .inner_margin(egui::Margin::symmetric(gap_lg() as i8, 0))
        .show(ui, |ui| {
            ui.set_min_height(title_h);
            ui.horizontal(|ui| {
                ui.set_min_height(title_h);
                // Title
                ui.label(
                    egui::RichText::new("THEME STUDIO")
                        .size(13.0)
                        .strong()
                        .color(t.text),
                );
                ui.add_space(gap_sm());
                // Pack name label
                ui.label(
                    egui::RichText::new(&pack.manifest.name)
                        .size(11.0)
                        .color(t.dim),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Close
                    if Button::new("×")
                        .variant(Variant::Ghost)
                        .size(Size::Md)
                        .show(ui, t)
                        .clicked()
                    {
                        close(ctx);
                    }

                    ui.add_space(gap_sm());

                    // Apply to app
                    if Button::new("Apply to App")
                        .variant(Variant::Primary)
                        .size(Size::Sm)
                        .show(ui, t)
                        .clicked()
                    {
                        apply_to_app(ctx, &pack);
                    }

                    ui.add_space(gap_sm());

                    // Export
                    if Button::new("Export .apextheme")
                        .variant(Variant::Secondary)
                        .size(Size::Sm)
                        .show(ui, t)
                        .clicked()
                    {
                        do_export(&pack);
                    }

                    ui.add_space(gap_xs());

                    // Import
                    if Button::new("Import")
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .show(ui, t)
                        .clicked()
                    {
                        if let Some(imported) = do_import() {
                            pack = imported;
                        }
                    }

                    ui.add_space(gap_xs());

                    // Export to Figma — emit the FigmaThemeExport JSON the PUSH
                    // plugin (figma-plugin/) consumes to scaffold a Figma file.
                    if Button::new("Export to Figma")
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .show(ui, t)
                        .clicked()
                    {
                        do_export_figma(&pack);
                    }

                    ui.add_space(gap_xs());

                    // Import from Figma — run the PULL transformer on a
                    // FigmaThemeExport JSON (plugin output / get_figma_data pull).
                    if Button::new("Import from Figma")
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .show(ui, t)
                        .clicked()
                    {
                        if let Some(imported) = do_import_figma() {
                            pack = imported;
                        }
                    }
                });
            });
        });

    // ── Body: two-column split ────────────────────────────────────────────────
    let editor_w = 320.0_f32;
    let catalog_w = total_w - editor_w;

    ui.horizontal(|ui| {
        ui.set_height(ui.available_height());
        // Left: catalog
        ui.allocate_ui_with_layout(
            egui::vec2(catalog_w, ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                draw_catalog(ui, ctx, t, &pack);
            },
        );

        // Divider
        let (_resp, painter) = ui.allocate_painter(
            egui::vec2(1.0, ui.available_height()),
            egui::Sense::hover(),
        );
        painter.vline(
            painter.clip_rect().left(),
            painter.clip_rect().y_range(),
            egui::Stroke::new(stroke_thin(), t.toolbar_border),
        );

        // Right: editor
        ui.allocate_ui_with_layout(
            egui::vec2(editor_w, ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                draw_editor(ui, ctx, t, &mut pack);
            },
        );
    });

    // Write pack back to egui memory.
    ctx.data_mut(|d| d.insert_temp(Id::new(KEY_PACK), pack));
}

// ── Catalog (storybook) ───────────────────────────────────────────────────────

fn draw_catalog(ui: &mut Ui, ctx: &Context, t: &Theme, pack: &WorkingPack) {
    // Build a preview PortableTheme from the working ColorScheme so widgets
    // rendered inside the catalog reflect current editor edits.
    let preview_t = color_scheme_to_portable(&pack.color_scheme);

    // Stash preview ambient. After this block we restore the app ambient
    // by stashing `t` back (the host will re-stash on the next frame).
    set_ambient_theme(ctx, preview_t.clone());
    set_ambient_recipes(ctx, Arc::new(RecipeSet::new()));

    PanelSection::new("CATALOG — COMPONENT STORYBOOK")
        .show(ui, t, |ui, _| {
            ScrollArea::vertical()
                .id_salt(KEY_CATALOG_SCROLL)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // ── Buttons ───────────────────────────────────────────
                    catalog_section(ui, t, "BUTTONS", |ui| {
                        let variants = [
                            (Variant::Primary,   "Primary"),
                            (Variant::Secondary, "Secondary"),
                            (Variant::Ghost,     "Ghost"),
                            (Variant::Danger,    "Danger"),
                            (Variant::Link,      "Link"),
                        ];
                        let sizes = [Size::Sm, Size::Md, Size::Lg];
                        for (v, v_label) in &variants {
                            ui.horizontal(|ui| {
                                ui.set_min_height(32.0);
                                ui.label(
                                    egui::RichText::new(*v_label)
                                        .size(10.0)
                                        .color(t.dim),
                                );
                                ui.add_space(gap_sm());
                                for sz in &sizes {
                                    // Button::new takes &str — use the variant label only
                                    Button::new(*v_label)
                                        .variant(*v)
                                        .size(*sz)
                                        .show(ui, t);
                                    ui.add_space(gap_xs());
                                }
                            });
                            ui.add_space(gap_xs());
                        }
                    });

                    // ── Tabs ──────────────────────────────────────────────
                    catalog_section(ui, t, "TABS", |ui| {
                        let mut tab_idx: usize = ctx
                            .data(|d| d.get_temp(Id::new("studio_tab_demo")))
                            .unwrap_or(0_usize);
                        Tabs::new(&mut tab_idx, &["Markets", "Orders", "Research", "Journal"])
                            .treatment(crate::ui_kit::widgets::tabs::TabTreatment::Card)
                            .show(ui, t);
                        ctx.data_mut(|d| d.insert_temp(Id::new("studio_tab_demo"), tab_idx));
                    });

                    // ── Tags & Badges ─────────────────────────────────────
                    catalog_section(ui, t, "TAGS & BADGES", |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap_xs();
                            Tag::new("Neutral").tone(TagTone::Neutral).show(ui, t);
                            Tag::new("Accent").tone(TagTone::Accent).show(ui, t);
                            Tag::new("Bull").tone(TagTone::Bull).show(ui, t);
                            Tag::new("Warn").tone(TagTone::Warn).show(ui, t);
                            ui.add_space(gap_md());
                            Badge::count(3).show(ui, t);
                            ui.add_space(gap_xs());
                            Badge::count(12).show(ui, t);
                        });
                    });

                    // ── PanelSection + PanelListRow ───────────────────────
                    catalog_section(ui, t, "PANEL SECTION + ROWS", |ui| {
                        PanelSection::new("POSITIONS").count(3).show(ui, t, |ui, t| {
                            let mut sel: Option<usize> = ctx
                                .data(|d| d.get_temp(Id::new(KEY_LIST_SEL)));
                            const ROW_IDS: &[&str] = &["row_0", "row_1", "row_2"];
                            let rows: &[(&str, &str, bool)] = &[
                                ("AAPL", "+$142.50", true),
                                ("TSLA", "-$38.20", false),
                                ("SPY",  "+$9.80",  true),
                            ];
                            for (i, (sym, pnl, dir)) in rows.iter().enumerate() {
                                let is_sel = sel == Some(i);
                                let pnl_color = if *dir { t.bull } else { t.bear };
                                let pnl_str = *pnl;
                                let resp = PanelListRow::new(ROW_IDS[i])
                                    .primary(sym)
                                    .secondary(pnl)
                                    .trailing(move |ui, _| {
                                        ui.label(
                                            egui::RichText::new(pnl_str)
                                                .size(10.0)
                                                .color(pnl_color),
                                        );
                                    })
                                    .selected(is_sel)
                                    .show(ui, t);
                                if resp.clicked() {
                                    sel = Some(i);
                                }
                            }
                            ctx.data_mut(|d| d.insert_temp(Id::new(KEY_LIST_SEL), sel));
                        });
                    });

                    // ── Tooltip ───────────────────────────────────────────
                    catalog_section(ui, t, "TOOLTIP", |ui| {
                        let btn = Button::new("Hover me")
                            .variant(Variant::Secondary)
                            .size(Size::Sm)
                            .show(ui, t);
                        Tooltip::new("This is a tooltip.\nTheme-aware shadow + surface.")
                            .show(ui, &btn, t);
                    });

                    // ── Input ─────────────────────────────────────────────
                    catalog_section(ui, t, "INPUT", |ui| {
                        let mut val: String = ctx
                            .data(|d| d.get_temp::<String>(Id::new(KEY_INPUT_A)))
                            .unwrap_or_default();
                        let resp = Input::new(&mut val)
                            .placeholder("Search symbol…")
                            .show(ui, t);
                        if resp.response.changed() {
                            ctx.data_mut(|d| d.insert_temp(Id::new(KEY_INPUT_A), val));
                        }
                    });
                });
        });

    // Restore app ambient theme so the rest of the frame uses the real theme.
    // (The host will re-stash the real theme on the next frame tick; this
    //  covers the remainder of the current frame's draw calls.)
    let app_portable = theme_to_portable(t);
    set_ambient_theme(ctx, app_portable);
}

// ── Editor ────────────────────────────────────────────────────────────────────

fn draw_editor(ui: &mut Ui, _ctx: &Context, t: &Theme, pack: &mut WorkingPack) {
    ScrollArea::vertical()
        .id_salt("theme_studio_editor_scroll")
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let inset = gap_md() as i8;

            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(inset, inset))
                .show(ui, |ui| {
                    // ── Manifest fields ───────────────────────────────────
                    PanelSection::new("MANIFEST").show(ui, t, |ui, t| {
                        editor_string_field(ui, t, "Name",    &mut pack.manifest.name);
                        editor_string_field(ui, t, "Author",  &mut pack.manifest.author);
                        editor_string_field(ui, t, "Version", &mut pack.manifest.version);

                        // Dark/light toggle
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Mode").size(10.0).color(t.dim),
                            );
                            ui.add_space(gap_sm());
                            let dark = pack.color_scheme.meta.is_dark;
                            if Button::toggle("Dark", dark).size(Size::Sm).show(ui, t).clicked() {
                                pack.color_scheme.meta.is_dark = true;
                                pack.manifest.is_dark = true;
                            }
                            if Button::toggle("Light", !dark).size(Size::Sm).show(ui, t).clicked() {
                                pack.color_scheme.meta.is_dark = false;
                                pack.manifest.is_dark = false;
                            }
                        });
                        ui.add_space(gap_xs());
                    });

                    // ── Color pickers ─────────────────────────────────────
                    PanelSection::new("COLOR SCHEME").show(ui, t, |ui, t| {
                        let cs = &mut pack.color_scheme;
                        color_row(ui, t, "bg",      &mut cs.bg);
                        color_row(ui, t, "surface",  &mut cs.surface);
                        color_row(ui, t, "text",     &mut cs.text);
                        color_row(ui, t, "dim",      &mut cs.dim);
                        color_row(ui, t, "border",   &mut cs.border);
                        color_row(ui, t, "accent",   &mut cs.accent);
                        color_row(ui, t, "bull",     &mut cs.bull);
                        color_row(ui, t, "bear",     &mut cs.bear);
                        color_row(ui, t, "warn",     &mut cs.warn);
                        color_row(ui, t, "shadow",   &mut cs.shadow);
                    });

                    // ── StyleSystem sliders ───────────────────────────────
                    PanelSection::new("STYLE SYSTEM").show(ui, t, |ui, t| {
                        let ss = &mut pack.style_system;
                        scale_row(ui, t, "radius lg",    &mut ss.radii.lg,          0.0..=32.0);
                        scale_row(ui, t, "radius sm",    &mut ss.radii.sm,          0.0..=16.0);
                        scale_row(ui, t, "stroke thick",  &mut ss.strokes.thick,     0.5..=4.0);
                        scale_row(ui, t, "stroke std",    &mut ss.strokes.std,       0.3..=3.0);
                        scale_row(ui, t, "spacing md",   &mut ss.spacing.md,         4.0..=32.0);
                        scale_row(ui, t, "spacing lg",   &mut ss.spacing.lg,         8.0..=48.0);
                    });

                    // ── Validation report ─────────────────────────────────
                    PanelSection::new("VALIDATION").show(ui, t, |ui, t| {
                        let temp_pack = ThemePack {
                            manifest:     pack.manifest.clone(),
                            color_scheme: pack.color_scheme.clone(),
                            style_system: pack.style_system.clone(),
                            recipes:      RecipeSet::new(),
                            shell_profile: None,
                            assets:       AssetRegistry::new(),
                        };
                        let report = validate(&temp_pack, AccessibilityMode::Standard);

                        if report.errors.is_empty() && report.warnings.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("{} No issues", crate::ui_kit::icons::Icon::CHECK))
                                    .size(11.0)
                                    .color(t.bull),
                            );
                        }

                        for f in &report.errors {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new(crate::ui_kit::icons::Icon::X)
                                        .size(10.0)
                                        .color(t.bear),
                                );
                                ui.label(
                                    egui::RichText::new(format!("[{}] {}", f.code, f.message))
                                        .size(10.0)
                                        .color(t.bear),
                                );
                            });
                        }
                        for f in &report.warnings {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new("!")
                                        .size(10.0)
                                        .color(t.warn),
                                );
                                ui.label(
                                    egui::RichText::new(format!("[{}] {}", f.code, f.message))
                                        .size(10.0)
                                        .color(t.warn),
                                );
                            });
                        }
                    });
                });
        });
}

// ── Color row helper ──────────────────────────────────────────────────────────

/// Render a labelled colour picker for one `Rgba` field.
fn color_row(ui: &mut Ui, t: &Theme, label: &str, rgba: &mut Rgba) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label).size(10.0).color(t.dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Use egui's built-in color button — small swatch that opens a picker.
            let mut c = Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
            if ui
                .color_edit_button_srgba(&mut c)
                .on_hover_text(label)
                .changed()
            {
                rgba[0] = c.r();
                rgba[1] = c.g();
                rgba[2] = c.b();
                rgba[3] = c.a();
            }
        });
    });
    ui.add_space(2.0);
}

// ── Scale slider helper ───────────────────────────────────────────────────────

fn scale_row(ui: &mut Ui, t: &Theme, label: &str, val: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(10.0).color(t.dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::DragValue::new(val)
                    .range(range)
                    .speed(0.1)
                    .suffix(" px"),
            );
        });
    });
    ui.add_space(2.0);
}

// ── String field helper ───────────────────────────────────────────────────────

fn editor_string_field(ui: &mut Ui, t: &Theme, label: &str, s: &mut String) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(10.0).color(t.dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut tmp = s.clone();
            let resp = Input::new(&mut tmp).show(ui, t);
            if resp.response.changed() {
                *s = tmp;
            }
        });
    });
    ui.add_space(2.0);
}

// ── Catalog section wrapper ───────────────────────────────────────────────────

fn catalog_section(ui: &mut Ui, t: &Theme, title: &str, body: impl FnOnce(&mut Ui)) {
    ui.add_space(gap_sm());
    ui.label(egui::RichText::new(title).size(9.0).color(t.dim).strong());
    ui.add_space(gap_xs());
    egui::Frame::new()
        .fill(t.toolbar_bg)
        .stroke(egui::Stroke::new(stroke_thin(), t.toolbar_border))
        .corner_radius(egui::CornerRadius::same(radius_sm() as u8))
        .inner_margin(egui::Margin::same(gap_md() as i8))
        .show(ui, |ui| {
            body(ui);
        });
}

// ── Apply / Export / Import ───────────────────────────────────────────────────

/// Push the working pack into the live renderer registries app-wide.
fn apply_to_app(ctx: &Context, pack: &WorkingPack) {
    let theme_pack = ThemePack {
        manifest:     pack.manifest.clone(),
        color_scheme: pack.color_scheme.clone(),
        style_system: pack.style_system.clone(),
        recipes:      RecipeSet::new(),
        shell_profile: None,
        assets:       AssetRegistry::new(),
    };
    crate::chart_renderer::theme_pack_bridge::activate_theme_pack(ctx, &theme_pack);
}

/// Export the working pack to a user-chosen `.apextheme` path via rfd.
fn do_export(pack: &WorkingPack) {
    let path_opt = rfd::FileDialog::new()
        .set_file_name(format!("{}.apextheme", pack.manifest.id))
        .add_filter("Apex Theme", &["apextheme"])
        .save_file();

    if let Some(path) = path_opt {
        let theme_pack = ThemePack {
            manifest:     pack.manifest.clone(),
            color_scheme: pack.color_scheme.clone(),
            style_system: pack.style_system.clone(),
            recipes:      RecipeSet::new(),
            shell_profile: None,
            assets:       AssetRegistry::new(),
        };
        if let Err(e) = theme_pack.write_bundle(&path) {
            eprintln!("[theme-studio] export error: {e}");
        }
    }
}

/// Import a `.apextheme` file selected via rfd, returning a populated WorkingPack.
fn do_import() -> Option<WorkingPack> {
    let path = rfd::FileDialog::new()
        .add_filter("Apex Theme", &["apextheme"])
        .pick_file()?;

    match ThemePack::read_bundle(&path) {
        Ok(tp) => Some(WorkingPack {
            manifest:     tp.manifest,
            color_scheme: tp.color_scheme,
            style_system: tp.style_system,
        }),
        Err(e) => {
            eprintln!("[theme-studio] import error: {e}");
            None
        }
    }
}

/// Export the working pack as a Figma-variables JSON (`FigmaThemeExport`) — the
/// shared contract the PUSH plugin (`figma-plugin/`) turns into Figma variables.
/// Single-mode export named after the working scheme/style.
fn do_export_figma(pack: &WorkingPack) {
    let path_opt = rfd::FileDialog::new()
        .set_file_name(format!("{}.figma.json", pack.manifest.id))
        .add_filter("Figma Theme Export", &["json"])
        .save_file();

    if let Some(path) = path_opt {
        let color_mode = pack.color_scheme.meta.name.clone();
        let style_mode = pack.style_system.meta.name.clone();
        let export = theme_to_figma_export(
            &[(color_mode, pack.color_scheme.clone())],
            &[(style_mode, pack.style_system.clone())],
            &BTreeMap::<String, RecipeSpec>::new(),
        );
        match serde_json::to_string_pretty(&export) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    eprintln!("[theme-studio] figma export write error: {e}");
                }
            }
            Err(e) => eprintln!("[theme-studio] figma export serialise error: {e}"),
        }
    }
}

/// Import a Figma-variables JSON (`FigmaThemeExport`) produced by the plugin (or
/// a `get_figma_data` pull) by running the PULL transformer into a WorkingPack.
/// Validation findings are logged; the pack still loads so the author can edit.
fn do_import_figma() -> Option<WorkingPack> {
    let path = rfd::FileDialog::new()
        .add_filter("Figma Theme Export", &["json"])
        .pick_file()?;

    let json = match std::fs::read_to_string(&path) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("[theme-studio] figma import read error: {e}");
            return None;
        }
    };
    let export: FigmaThemeExport = match serde_json::from_str(&json) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[theme-studio] figma import parse error: {e}");
            return None;
        }
    };
    match figma_export_to_pack(&export, &ImportOptions::default(), &FigmaMapping::identity()) {
        Ok(tp) => {
            let report = validate(&tp, AccessibilityMode::Standard);
            if !report.is_installable() {
                eprintln!(
                    "[theme-studio] figma import validation: {report}; errors: {:?}",
                    report.errors
                );
            }
            Some(WorkingPack {
                manifest:     tp.manifest,
                color_scheme: tp.color_scheme,
                style_system: tp.style_system,
            })
        }
        Err(e) => {
            eprintln!("[theme-studio] figma import transform error: {e}");
            None
        }
    }
}

// ── Theme conversion helpers ──────────────────────────────────────────────────

/// Build a `PortableTheme` from a `ColorScheme` for the catalog live-preview.
fn color_scheme_to_portable(cs: &ColorScheme) -> PortableTheme {
    let c32 = |r: Rgba| -> Color32 {
        Color32::from_rgba_unmultiplied(r[0], r[1], r[2], r[3])
    };
    let bg_c   = c32(cs.bg);
    let text_c = c32(cs.text);
    let acc_c  = c32(cs.accent);
    let is_dark = cs.meta.is_dark;
    let alpha   = |base: Color32, a: u8| Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), a);

    PortableTheme {
        accent:           acc_c,
        bull:             c32(cs.bull),
        bear:             c32(cs.bear),
        text:             text_c,
        dim:              c32(cs.dim),
        border:           c32(cs.border),
        border_variant:   c32(cs.border),
        warn:             c32(cs.warn),
        bg:               bg_c,
        surface:          c32(cs.surface),
        element_hover:    if is_dark { alpha(Color32::WHITE, 14) } else { alpha(Color32::BLACK, 14) },
        element_active:   if is_dark { alpha(Color32::WHITE, 28) } else { alpha(Color32::BLACK, 28) },
        element_selected: alpha(acc_c, 40),
        element_disabled: if is_dark { alpha(Color32::WHITE,  8) } else { alpha(Color32::BLACK,  8) },
        ghost_hover:      if is_dark { alpha(Color32::WHITE, 10) } else { alpha(Color32::BLACK, 10) },
        ghost_active:     if is_dark { alpha(Color32::WHITE, 22) } else { alpha(Color32::BLACK, 22) },
        icon:             text_c,
        icon_muted:       c32(cs.dim),
        icon_disabled:    c32(cs.dim).gamma_multiply(0.5),
        icon_accent:      acc_c,
        shadow_color:     c32(cs.shadow),
    }
}

/// Build a `PortableTheme` from the concrete `gpu::Theme` for ambient restore.
/// Derived values (element_*, ghost_*) mirror `theme_impl.rs` exactly.
fn theme_to_portable(t: &Theme) -> PortableTheme {
    let ca = |c: Color32, a: u8| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a);
    PortableTheme {
        accent:           t.accent,
        bull:             t.bull,
        bear:             t.bear,
        text:             t.text,
        dim:              t.dim,
        border:           t.toolbar_border,
        border_variant:   t.border_variant,
        warn:             t.warn,
        bg:               t.bg,
        surface:          t.toolbar_bg,
        element_hover:    ca(t.text,   12),
        element_active:   ca(t.text,   24),
        element_selected: ca(t.accent, 24),
        element_disabled: ca(t.dim,    80),
        ghost_hover:      ca(t.text,    6),
        ghost_active:     ca(t.text,   12),
        icon:             t.text,
        icon_muted:       ca(t.text,  178),
        icon_disabled:    ca(t.text,  102),
        icon_accent:      t.accent,
        shadow_color:     t.shadow_color,
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::{
        color_scheme::builtin_dark,
        theme_pack::{
            manifest::ThemeManifest,
            validate::{validate, AccessibilityMode},
        },
    };

    /// WorkingPack::default() builds cleanly and validates as installable.
    #[test]
    fn working_pack_default_validates() {
        let pack = WorkingPack::default();
        let theme_pack = ThemePack {
            manifest:     pack.manifest.clone(),
            color_scheme: pack.color_scheme.clone(),
            style_system: pack.style_system.clone(),
            recipes:      RecipeSet::new(),
            shell_profile: None,
            assets:       AssetRegistry::new(),
        };
        let report = validate(&theme_pack, AccessibilityMode::Standard);
        // The default builtin_dark() should be installable.
        assert!(
            report.is_installable(),
            "WorkingPack::default() should produce an installable pack; errors: {:?}",
            report.errors,
        );
    }

    /// color_scheme_to_portable round-trips accent/bull/bear correctly.
    #[test]
    fn color_scheme_to_portable_round_trips_semantic() {
        let cs = builtin_dark();
        let pt = color_scheme_to_portable(&cs);
        assert_eq!(pt.accent, Color32::from_rgba_unmultiplied(cs.accent[0], cs.accent[1], cs.accent[2], cs.accent[3]));
        assert_eq!(pt.bull,   Color32::from_rgba_unmultiplied(cs.bull[0],   cs.bull[1],   cs.bull[2],   cs.bull[3]));
        assert_eq!(pt.bear,   Color32::from_rgba_unmultiplied(cs.bear[0],   cs.bear[1],   cs.bear[2],   cs.bear[3]));
    }

    /// Export round-trip: build a WorkingPack, write_bundle, read_bundle, compare.
    #[test]
    fn studio_export_round_trip() {
        let pack = WorkingPack::default();
        let theme_pack = ThemePack {
            manifest:     pack.manifest.clone(),
            color_scheme: pack.color_scheme.clone(),
            style_system: pack.style_system.clone(),
            recipes:      RecipeSet::new(),
            shell_profile: None,
            assets:       AssetRegistry::new(),
        };
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        theme_pack.write_bundle(tmp.path()).expect("write_bundle");
        let loaded = ThemePack::read_bundle(tmp.path()).expect("read_bundle");

        assert_eq!(loaded.manifest.id,   theme_pack.manifest.id);
        assert_eq!(loaded.color_scheme.accent, theme_pack.color_scheme.accent);
        assert_eq!(loaded.color_scheme.bull,   theme_pack.color_scheme.bull);
        assert_eq!(loaded.color_scheme.bear,   theme_pack.color_scheme.bear);
        assert_eq!(loaded.style_system.radii.lg, theme_pack.style_system.radii.lg);
    }
}
