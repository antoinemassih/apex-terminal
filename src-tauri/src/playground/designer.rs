//! Theme Designer — palette editor + save/load that lands in the same
//! `themes_dir/colorschemes/*.json` folder the trading app's hot-reload
//! watcher polls. Themes you create here automatically appear in the
//! trading app's theme picker within ~1.5s.
//!
//! Design goals:
//! - Edit every ColorScheme palette field via an inline color picker.
//! - "Save As..." names the theme + writes DTCG JSON to the watch dir.
//! - "Load..." dropdown lists saved themes for fast iteration.
//! - "Reset to <builtin>" lets you fork a baseline (Midnight / Bauhaus /
//!   PortableTheme dark / PortableTheme light) without losing previous work.
//! - Live preview — every story in the playground re-renders next frame
//!   with the edited palette.
//!
//! Limited to ColorScheme (palette) for now. Adding StyleSystem (radii,
//! strokes, typography, treatments) editing is a follow-up — needs
//! DesignTokens integration which the playground doesn't currently use.

use _scaffold_lib::design_system::ColorScheme;
use _scaffold_lib::ui_kit::widgets::theme::{ComponentTheme, PortableTheme};
use egui::Color32;
use std::path::PathBuf;

// ── State ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesignerTab {
    /// Edit the ColorScheme palette (the 11 required + accent etc.).
    /// Saves to themes_dir/colorschemes/<name>.json (picked up by the
    /// trading app's hot-reload watcher).
    Palette,
    /// Embed the full F12 design inspector — every DesignTokens field
    /// (fonts, spacing, radii, strokes, alphas, shadows, component
    /// tokens). Mutates the global DesignTokens RwLock so changes
    /// propagate to all playground widgets via TokenSnapshot next frame.
    /// Only available when built with `--features design-mode`.
    #[cfg(feature = "design-mode")]
    Tokens,
}

impl DesignerTab {
    pub fn label(self) -> &'static str {
        match self {
            DesignerTab::Palette => "Palette",
            #[cfg(feature = "design-mode")]
            DesignerTab::Tokens => "Tokens",
        }
    }

    pub fn all() -> &'static [Self] {
        #[cfg(feature = "design-mode")]
        {
            &[DesignerTab::Palette, DesignerTab::Tokens]
        }
        #[cfg(not(feature = "design-mode"))]
        {
            &[DesignerTab::Palette]
        }
    }
}

pub struct DesignerState {
    /// The palette being edited. Source of truth for the playground's
    /// rendered theme. Converted to PortableTheme each frame.
    pub scheme: ColorScheme,
    /// True if the right-side designer panel is open.
    pub open: bool,
    /// Which sub-editor is showing (Palette vs Tokens).
    pub tab: DesignerTab,
    /// True if the Save As dialog is open.
    pub save_dialog_open: bool,
    /// Working name in the Save As dialog.
    pub save_name: String,
    /// True if the Load dialog is open.
    pub load_dialog_open: bool,
    /// Last action status — shown as a tiny line under the buttons
    /// ("Saved as foo.json", "Load failed: ..."), cleared by next interaction.
    pub status: String,
    /// Cached list of saved theme file stems, refreshed on directory open.
    pub saved_list: Vec<String>,
    /// The embedded F12 inspector. Lazily constructed on first Tokens-tab
    /// open. Mutates the global DesignTokens RwLock.
    #[cfg(feature = "design-mode")]
    pub inspector: Option<_scaffold_lib::design_inspector::Inspector>,
}

impl Default for DesignerState {
    fn default() -> Self {
        let scheme = builtin_starting_scheme();
        Self {
            scheme,
            open: false,
            tab: DesignerTab::Palette,
            save_dialog_open: false,
            save_name: String::new(),
            load_dialog_open: false,
            status: String::new(),
            saved_list: Vec::new(),
            #[cfg(feature = "design-mode")]
            inspector: None,
        }
    }
}

// ── Starting schemes ─────────────────────────────────────────────────────────
//
// Wraps the design_system module's builtin starting points so the playground
// designer always has a sensible "fork from here" baseline.

pub fn builtin_starting_scheme() -> ColorScheme {
    _scaffold_lib::design_system::color_scheme::builtin_dark()
}

pub fn builtin_light_scheme() -> ColorScheme {
    _scaffold_lib::design_system::color_scheme::builtin_light()
}

// ── ColorScheme → PortableTheme conversion ───────────────────────────────────
//
// PortableTheme has more fields than ColorScheme (element_*, ghost_*,
// icon_*, border_variant) — those are derived from the base palette
// here so the playground can render every widget with a single edit
// surface (the 11-field ColorScheme palette).

/// ColorScheme stores colors as `Rgba = [u8; 4]`; convert to egui's Color32.
fn rgba_to_c(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

pub fn color_scheme_to_portable(cs: &ColorScheme) -> PortableTheme {
    let text = rgba_to_c(cs.text);
    let accent = rgba_to_c(cs.accent);
    let border = rgba_to_c(cs.border);
    let dim = rgba_to_c(cs.dim);
    PortableTheme {
        accent,
        text,
        dim,
        border,
        // Slightly punchier variant — used by inputs/cards for the second-
        // tier outline. Approximated as +20% alpha over border in dark
        // themes, +10% darker in light themes.
        border_variant: if cs.meta.is_dark {
            blend_toward(border, text, 0.20)
        } else {
            blend_toward(border, Color32::BLACK, 0.10)
        },
        warn: rgba_to_c(cs.warn),
        bg: rgba_to_c(cs.bg),
        surface: rgba_to_c(cs.surface),
        // Hover/active/selected/disabled tints derived from text/accent.
        element_hover:    Color32::from_rgba_unmultiplied(text.r(), text.g(), text.b(), 14),
        element_active:   Color32::from_rgba_unmultiplied(text.r(), text.g(), text.b(), 28),
        element_selected: Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 44),
        element_disabled: Color32::from_rgba_unmultiplied(text.r(), text.g(), text.b(),  8),
        ghost_hover:      Color32::from_rgba_unmultiplied(text.r(), text.g(), text.b(), 10),
        ghost_active:     Color32::from_rgba_unmultiplied(text.r(), text.g(), text.b(), 22),
        icon: text,
        icon_muted: dim,
        icon_disabled: border,
        icon_accent: accent,
        shadow_color: rgba_to_c(cs.shadow),
    }
}

fn blend_toward(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp = |x: u8, y: u8| -> u8 { (x as f32 + (y as f32 - x as f32) * t).clamp(0.0, 255.0) as u8 };
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

// ── Disk I/O — DTCG JSON in the trading app's themes_dir ─────────────────────

/// Where saved playground themes go. Same directory the trading app's
/// hot-reload watcher polls every 1.5s, so themes saved here appear in
/// the trading app's theme picker.
pub fn save_dir() -> Option<PathBuf> {
    let dir = _scaffold_lib::design_system::themes_dir()?;
    Some(dir.join("colorschemes"))
}

pub fn save_scheme(scheme: &ColorScheme, name: &str) -> Result<PathBuf, String> {
    let dir = save_dir().ok_or_else(|| "themes_dir not initialised".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    let safe_name = sanitize_filename(name);
    if safe_name.is_empty() {
        return Err("name required".to_string());
    }
    let mut scheme = scheme.clone();
    scheme.meta.id = safe_name.clone();
    scheme.meta.name = name.to_string();
    let path = dir.join(format!("{safe_name}.json"));
    let json = scheme.to_dtcg();
    std::fs::write(&path, json).map_err(|e| format!("write: {e}"))?;
    Ok(path)
}

pub fn load_scheme(name_stem: &str) -> Result<ColorScheme, String> {
    let dir = save_dir().ok_or_else(|| "themes_dir not initialised".to_string())?;
    let path = dir.join(format!("{name_stem}.json"));
    let json = std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
    ColorScheme::from_dtcg(&json).map_err(|e| format!("parse: {e}"))
}

pub fn list_saved() -> Vec<String> {
    let Some(dir) = save_dir() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("json") {
                p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();
    out.sort();
    out
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else if c.is_whitespace() {
                '_'
            } else {
                '_'
            }
        })
        .collect()
}

// ── Designer panel UI ────────────────────────────────────────────────────────

pub fn render_designer_panel(
    ui: &mut egui::Ui,
    state: &mut DesignerState,
    theme: &PortableTheme,
) {
    use _scaffold_lib::ui_kit::widgets::{Button, Variant, Size};
    let dim = theme.dim();
    let text = theme.text();

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("THEME DESIGNER")
            .color(text)
            .size(13.0)
            .strong(),
    );
    ui.label(
        egui::RichText::new(
            "Edits the active palette + design tokens in real time. Palette saves land in the trading app's themes_dir.",
        )
        .color(dim)
        .size(10.0),
    );
    ui.add_space(10.0);

    // ── Tab strip — explicit "what am I editing?" indicator ──────────────
    ui.label(
        egui::RichText::new("EDITING:")
            .color(dim)
            .size(9.5)
            .strong(),
    );
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for tab in DesignerTab::all() {
            let active = state.tab == *tab;
            // Primary when active, Ghost when inactive — high contrast so
            // the tabs are impossible to miss.
            let variant = if active { Variant::Primary } else { Variant::Ghost };
            let label = match tab {
                DesignerTab::Palette => "🎨  Palette (colours)",
                #[cfg(feature = "design-mode")]
                DesignerTab::Tokens => "📐  Tokens (sizes/spacing/etc.)",
            };
            if Button::new(label)
                .variant(variant)
                .size(Size::Sm)
                .show(ui, theme)
                .clicked()
            {
                state.tab = *tab;
            }
        }
    });
    #[cfg(not(feature = "design-mode"))]
    {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Tokens tab is unavailable — rebuild with --features design-mode to edit sizes / spacing / radii / strokes / alphas / etc.",
            )
            .color(dim)
            .size(9.5),
        );
    }
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    // ── Tab-specific body ─────────────────────────────────────────────────
    match state.tab {
        DesignerTab::Palette => render_palette_tab(ui, state, theme),
        #[cfg(feature = "design-mode")]
        DesignerTab::Tokens => render_tokens_tab(ui, state, theme),
    }
}

#[cfg(feature = "design-mode")]
fn render_tokens_tab(
    ui: &mut egui::Ui,
    state: &mut DesignerState,
    _theme: &PortableTheme,
) {
    // Lazy init — the Inspector lives across frames so its category
    // selection / scroll state / open-help-panel flag persists.
    if state.inspector.is_none() {
        let mut insp = _scaffold_lib::design_inspector::Inspector::new(
            std::path::PathBuf::from("design.toml"),
        );
        insp.themes_dir = _scaffold_lib::design_system::themes_dir();
        // Open by default — the user clicked the Tokens tab so they
        // want to see the editor.
        insp.open = true;
        state.inspector = Some(insp);
    }

    // The inspector mutates DesignTokens. Reentrancy-safe pattern: clone
    // the global into a local, let the inspector mutate, write back if
    // it changed. (Holding the RwLock write across the inspector body
    // would deadlock if any widget inside reads via dt_f32!.)
    if let Some(insp) = state.inspector.as_mut() {
        if let Some(mut tokens_local) = _scaffold_lib::design_tokens::get() {
            let mut modified = false;
            insp.show_inspector_body(ui, &mut tokens_local, &mut modified);
            if modified {
                _scaffold_lib::design_tokens::update(tokens_local);
            }
        } else {
            // DesignTokens hasn't been initialized yet — initialize with
            // defaults so the playground can edit it.
            _scaffold_lib::design_tokens::init(
                _scaffold_lib::design_tokens::DesignTokens::default(),
            );
            ui.label("Initializing design tokens — refreshing next frame...");
            ui.ctx().request_repaint();
        }
    }
}

fn render_palette_tab(
    ui: &mut egui::Ui,
    state: &mut DesignerState,
    theme: &PortableTheme,
) {
    use _scaffold_lib::ui_kit::widgets::{Button, Variant, Size};
    let dim = theme.dim();
    let text = theme.text();

    // ── Action buttons row ─────────────────────────────────────────────────
    ui.horizontal(|ui| {
        if Button::new("Save As…").variant(Variant::Primary).size(Size::Xs).show(ui, theme).clicked() {
            state.save_dialog_open = true;
            if state.save_name.is_empty() {
                state.save_name = state.scheme.meta.name.clone();
            }
            state.status.clear();
        }
        if Button::new("Load…").variant(Variant::Ghost).size(Size::Xs).show(ui, theme).clicked() {
            state.saved_list = list_saved();
            state.load_dialog_open = true;
            state.status.clear();
        }
        if Button::new("Reset → Dark").variant(Variant::Ghost).size(Size::Xs).show(ui, theme).clicked() {
            state.scheme = builtin_starting_scheme();
            state.status = "Reset to builtin dark".to_string();
        }
        if Button::new("Reset → Light").variant(Variant::Ghost).size(Size::Xs).show(ui, theme).clicked() {
            state.scheme = builtin_light_scheme();
            state.status = "Reset to builtin light".to_string();
        }
    });

    if !state.status.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(&state.status).color(dim).size(9.5));
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    // ── Palette editor — every required ColorScheme field ─────────────────
    let mut is_dark = state.scheme.meta.is_dark;
    color_row(ui, "bg",      &mut state.scheme.bg,      "App background", text, dim);
    color_row(ui, "surface", &mut state.scheme.surface, "Panel surface fills", text, dim);
    ui.add_space(6.0);
    color_row(ui, "text",    &mut state.scheme.text,    "Body text", text, dim);
    color_row(ui, "dim",     &mut state.scheme.dim,     "Secondary/dim text", text, dim);
    color_row(ui, "border",  &mut state.scheme.border,  "Borders, outlines, dividers", text, dim);
    ui.add_space(6.0);
    color_row(ui, "accent",  &mut state.scheme.accent,  "Primary action, links, focus", text, dim);
    color_row(ui, "bull",    &mut state.scheme.bull,    "Buy/up moves, gain", text, dim);
    color_row(ui, "bear",    &mut state.scheme.bear,    "Sell/down moves, loss", text, dim);
    color_row(ui, "warn",    &mut state.scheme.warn,    "Pending state, warnings", text, dim);
    color_row(ui, "shadow",  &mut state.scheme.shadow,  "Theme-aware shadow color", text, dim);

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        let label = if is_dark { "Dark theme" } else { "Light theme" };
        if ui.checkbox(&mut is_dark, label).changed() {
            state.scheme.meta.is_dark = is_dark;
        }
    });
    ui.label(egui::RichText::new("Affects how border_variant + element states are derived.").color(dim).size(9.5));

    // ── Save As dialog ────────────────────────────────────────────────────
    if state.save_dialog_open {
        egui::Window::new("Save As")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label("Theme name:");
                ui.text_edit_singleline(&mut state.save_name);
                ui.label(
                    egui::RichText::new(
                        "Will be saved as colorschemes/<sanitized-name>.json — appears in the trading app's theme picker.",
                    )
                    .size(9.5)
                    .color(theme.dim()),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if Button::new("Save").variant(Variant::Primary).size(Size::Sm).show(ui, theme).clicked() {
                        match save_scheme(&state.scheme, &state.save_name) {
                            Ok(p) => {
                                state.status = format!("Saved: {}", p.file_name().and_then(|s| s.to_str()).unwrap_or("?"));
                                state.save_dialog_open = false;
                            }
                            Err(e) => state.status = format!("Save failed: {e}"),
                        }
                    }
                    if Button::new("Cancel").variant(Variant::Ghost).size(Size::Sm).show(ui, theme).clicked() {
                        state.save_dialog_open = false;
                    }
                });
            });
    }

    // ── Load dialog ───────────────────────────────────────────────────────
    if state.load_dialog_open {
        egui::Window::new("Load Theme")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                if state.saved_list.is_empty() {
                    ui.label("No saved themes yet. Create one with Save As…");
                } else {
                    ui.label("Pick a theme to load:");
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        for name in &state.saved_list.clone() {
                            if Button::new(name.as_str()).variant(Variant::Ghost).size(Size::Sm).show(ui, theme).clicked() {
                                match load_scheme(name) {
                                    Ok(loaded) => {
                                        state.scheme = loaded;
                                        state.status = format!("Loaded: {name}");
                                        state.load_dialog_open = false;
                                    }
                                    Err(e) => state.status = format!("Load failed: {e}"),
                                }
                            }
                        }
                    });
                }
                ui.add_space(6.0);
                if Button::new("Cancel").variant(Variant::Ghost).size(Size::Sm).show(ui, theme).clicked() {
                    state.load_dialog_open = false;
                }
            });
    }
}

/// `color` is `Rgba = [u8; 4]` from the design_system module.
fn color_row(
    ui: &mut egui::Ui,
    label: &str,
    color: &mut [u8; 4],
    description: &str,
    text: Color32,
    dim: Color32,
) {
    ui.horizontal(|ui| {
        let mut rgba = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
        ];
        if ui.color_edit_button_rgb(&mut rgba).changed() {
            color[0] = (rgba[0] * 255.0) as u8;
            color[1] = (rgba[1] * 255.0) as u8;
            color[2] = (rgba[2] * 255.0) as u8;
            color[3] = 255;
        }
        ui.label(egui::RichText::new(label).monospace().size(11.0).color(text));
    });
    ui.label(egui::RichText::new(description).size(9.5).color(dim));
    ui.add_space(2.0);
}
