//! DTCG export and directory-scan helpers for the built-in theme catalogue.
//!
//! ## Functions
//!
//! - [`export_builtin_themes`] — write every built-in [`ColorScheme`] and
//!   [`StyleSystem`] to `<dir>/colorschemes/<id>.json` /
//!   `<dir>/styles/<id>.json` using DTCG serialization.
//! - [`scan_theme_dir`] — read back any `colorschemes/*.json` and
//!   `styles/*.json` files in a directory tree, skipping files that fail to
//!   parse (graceful, no panic).
//!
//! Neither function runs at startup.  `export_builtin_themes` is called on
//! demand (e.g. from a menu action or CLI flag); `scan_theme_dir` is the
//! "installed themes" scan path for the registry.

use std::{
    fs,
    io::{self, Write},
    path::Path,
};

use serde_json::{json, Value};

use super::{
    builtin::{builtin_color_schemes, builtin_style_systems},
    color_scheme::ColorScheme,
    loader::LoadError,
    style_system::{BevelStyle, FocusRingStyle, GroupEnclosure, PaneActiveIndicator, StyleSystem},
};

// ── StyleSystem::to_dtcg ─────────────────────────────────────────────────────

impl StyleSystem {
    /// Serialize this `StyleSystem` into a W3C DTCG JSON string.
    ///
    /// Mirrors `ColorScheme::to_dtcg`.  Produces the `style.*.json` shape
    /// described in the loader module comment.
    pub fn to_dtcg(&self) -> String {
        macro_rules! dim {
            ($v:expr) => {
                json!({ "$type": "dimension", "$value": $v })
            };
        }
        macro_rules! bool_tok {
            ($v:expr) => {
                json!({ "$type": "boolean", "$value": $v })
            };
        }

        macro_rules! num {
            ($v:expr) => {
                json!({ "$type": "number", "$value": $v })
            };
        }
        macro_rules! int_tok {
            ($v:expr) => {
                json!({ "$type": "integer", "$value": $v })
            };
        }
        macro_rules! str_tok {
            ($v:expr) => {
                json!({ "$type": "string", "$value": $v })
            };
        }

        let typ = &self.typography;
        let sp  = &self.spacing;
        let r   = &self.radii;
        let st  = &self.strokes;
        let al  = &self.alphas;
        let el  = &self.elevation;
        let den = &self.density;
        let ico = &self.icons;
        let lh  = &self.line_heights;
        let sh  = &self.shadows;
        let tr  = &self.treatments;
        let ch  = &self.chrome;
        let shl = &self.shell;   // DS-6.0 (`sh` is already shadows)

        let focus_ring_str = match tr.focus_ring {
            FocusRingStyle::None    => "none",
            FocusRingStyle::Outline => "outline",
            FocusRingStyle::Glow    => "glow",
        };

        let bevel_str = match tr.surface_bevel {
            BevelStyle::None   => "none",
            BevelStyle::Raised => "raised",
            BevelStyle::Inset  => "inset",
        };

        let pane_indicator_str = match PaneActiveIndicator::from_u8(ch.pane_active_indicator) {
            PaneActiveIndicator::None       => "none",
            PaneActiveIndicator::TopStripe  => "top_stripe",
            PaneActiveIndicator::HeaderFill => "header_fill",
            PaneActiveIndicator::Both       => "both",
        };

        let group_enclosure_str = match ch.button_group {
            GroupEnclosure::None     => "none",
            GroupEnclosure::Bordered => "bordered",
            GroupEnclosure::Frosted  => "frosted",
            GroupEnclosure::Sharp    => "sharp",
        };

        let shadow_obj = |s: &super::style_system::ShadowSpec| -> Value {
            json!({
                "blur":     { "$type": "dimension", "$value": s.blur },
                "spread":   { "$type": "dimension", "$value": s.spread },
                "offset_x": { "$type": "dimension", "$value": s.offset_x },
                "offset_y": { "$type": "dimension", "$value": s.offset_y },
                "alpha":    { "$type": "number",    "$value": s.alpha },
            })
        };

        // One rung of the elevation ladder. `radius` is Gaussian sigma, not a
        // corner radius — deliberately a different shape from `shadow_obj`.
        let tier_obj = |t: &super::style_system::ShadowTier| -> Value {
            json!({
                "radius":   { "$type": "dimension", "$value": t.radius },
                "offset_y": { "$type": "dimension", "$value": t.offset_y },
                "alpha":    { "$type": "number",    "$value": t.alpha },
            })
        };

        let root = json!({
            "meta": {
                "id":      self.meta.id,
                "name":    self.meta.name,
                "is_dark": self.meta.is_dark,
            },
            "typography": {
                "size_xs": dim!(typ.size_xs),
                "size_sm": dim!(typ.size_sm),
                "size_md": dim!(typ.size_md),
                "size_lg": dim!(typ.size_lg),
                "size_xl": dim!(typ.size_xl),
                "mono_sm": dim!(typ.mono_sm),
                "mono_md": dim!(typ.mono_md),
                "mono_lg": dim!(typ.mono_lg),
                "size_section_label": dim!(typ.size_section_label),
                "label_tracking": num!(typ.label_tracking),
                "nav_tracking":   num!(typ.nav_tracking),
                "section_tracking": num!(typ.section_tracking),
                "family_ui":      str_tok!(&typ.family_ui),
                "family_mono":    str_tok!(&typ.family_mono),
                "family_display": str_tok!(&typ.family_display),
            },
            "spacing": {
                "xs":              dim!(sp.xs),
                "sm":              dim!(sp.sm),
                "xs_mid":          dim!(sp.xs_mid),
                "md":              dim!(sp.md),
                "lg":              dim!(sp.lg),
                "xl":              dim!(sp.xl),
                "xxl":             dim!(sp.xxl),
                "gmd":             dim!(sp.gmd),
                "cta_height":      dim!(sp.cta_height),
                "cta_padding_x":   dim!(sp.cta_padding_x),
                "button_height":   dim!(sp.button_height),
                "button_padding_x":dim!(sp.button_padding_x),
                "tab_height":      dim!(sp.tab_height),
            },
            "radii": {
                "none": dim!(r.none),
                "xs":   dim!(r.xs),
                "sm":   dim!(r.sm),
                "md":   dim!(r.md),
                "lg":   dim!(r.lg),
                "full": dim!(r.full),
                "pill": dim!(r.pill),
                "chip": dim!(r.chip),
            },
            "strokes": {
                "hair":   dim!(st.hair),
                "thin":   dim!(st.thin),
                "medium": dim!(st.medium),
                "std":    dim!(st.std),
                "bold":   dim!(st.bold),
                "thick":  dim!(st.thick),
                "md":     dim!(st.md),
                "heavy":  dim!(st.heavy),
            },
            "alphas": {
                // u8 tiers
                "faint":     int_tok!(al.faint),
                "ghost":     int_tok!(al.ghost),
                "soft_u8":   int_tok!(al.soft_u8),
                "subtle_u8": int_tok!(al.subtle_u8),
                "tint":      int_tok!(al.tint),
                "muted_u8":  int_tok!(al.muted_u8),
                "dim":       int_tok!(al.dim),
                "line":      int_tok!(al.line),
                "strong_u8": int_tok!(al.strong_u8),
                "active":    int_tok!(al.active),
                "heavy_u8":  int_tok!(al.heavy_u8),
                "scrim":     int_tok!(al.scrim),
                "dense":      int_tok!(al.dense),
                "near_solid": int_tok!(al.near_solid),
                "solid":     int_tok!(al.solid),
                // f32 multipliers
                "subtle":        num!(al.subtle),
                "soft":          num!(al.soft),
                "muted":         num!(al.muted),
                "mid":           num!(al.mid),
                "strong":        num!(al.strong),
                "opaque":        num!(al.opaque),
                "header_border": num!(al.header_border),
            },
            "elevation": {
                "l1": num!(el.l1),
                "l2": num!(el.l2),
                "l3": num!(el.l3),
            },
            "density": {
                "factor":                 num!(den.factor),
                "row_height_dense":       dim!(den.row_height_dense),
                "row_height_comfortable": dim!(den.row_height_comfortable),
                // The STRUCTURAL ladder. None of this was exported, and the
                // loader hardcoded it — so the row heights, splitter, rails and
                // control heights were unauthorable by any theme pack. That is
                // the mechanical reason "themes can breathe gutters, not
                // proportions": the proportions had no way in or out.
                "row_dense":              dim!(den.row_dense),
                "row_compact":            dim!(den.row_compact),
                "row_default":            dim!(den.row_default),
                "row_spacious":           dim!(den.row_spacious),
                "row_tall":               dim!(den.row_tall),
                "splitter_width":         dim!(den.splitter_width),
                "rail_narrow":            dim!(den.rail_narrow),
                "rail_medium":            dim!(den.rail_medium),
                "rail_wide":              dim!(den.rail_wide),
                "control_xs":             dim!(den.control_xs),
                "control_sm":             dim!(den.control_sm),
                "control_md":             dim!(den.control_md),
                "control_lg":             dim!(den.control_lg),
                "control_xl":             dim!(den.control_xl),
            },
            // Icons and leading. Added with the token groups themselves — a
            // token that cannot survive an export/import round trip is only
            // half-authorable, and the round-trip test caught exactly that:
            // Aperture's authored 19/22 icons and 1.15..1.45 leading came back
            // as the defaults.
            "icons": {
                "xs": dim!(ico.xs),
                "sm": dim!(ico.sm),
                "md": dim!(ico.md),
                "lg": dim!(ico.lg),
            },
            "line_heights": {
                "tight":   num!(lh.tight),
                "heading": num!(lh.heading),
                "dense":   num!(lh.dense),
                "compact": num!(lh.compact),
                "normal":  num!(lh.normal),
                "loose":   num!(lh.loose),
            },
            "shadows": {
                "card":     shadow_obj(&sh.card),
                "modal":    shadow_obj(&sh.modal),
                "tooltip":  shadow_obj(&sh.tooltip),
                // The elevation ladder. A pack that omits this still loads with
                // the ladder it was authored against — but one that TUNES it
                // must round-trip, and the export link is the one this chain
                // has silently dropped three separate times.
                "tiers": {
                    "sm": tier_obj(&sh.tiers.sm),
                    "md": tier_obj(&sh.tiers.md),
                    "lg": tier_obj(&sh.tiers.lg),
                    "xl": tier_obj(&sh.tiers.xl),
                },
            },
            "treatments": {
                "solid_active_fills":         bool_tok!(tr.solid_active_fills),
                "hairline_borders":           bool_tok!(tr.hairline_borders),
                "uppercase_section_labels":   bool_tok!(tr.uppercase_section_labels),
                "numbered_section_labels":    bool_tok!(tr.numbered_section_labels),
                "segmented_filled_idle":      bool_tok!(tr.segmented_filled_idle),
                "focus_ring":                 str_tok!(focus_ring_str),
                // Previously-defaulted fields — now fully round-tripped
                "surface_bevel":              str_tok!(bevel_str),
                "bevel_highlight_alpha":      int_tok!(tr.bevel_highlight_alpha),
                "bevel_shadow_alpha":         int_tok!(tr.bevel_shadow_alpha),
                "wl_row_side_margin":         dim!(tr.wl_row_side_margin),
                "wl_row_corner_radius":       int_tok!(tr.wl_row_corner_radius),
                "wl_row_divider_alpha":       int_tok!(tr.wl_row_divider_alpha),
                "section_header_mono":        bool_tok!(tr.section_header_mono),
                "wl_symbol_mono":             bool_tok!(tr.wl_symbol_mono),
                "panel_tab_treatment":        int_tok!(tr.panel_tab_treatment),
                "pane_active_fill_accent":    bool_tok!(tr.pane_active_fill_accent),
                "serif_headlines":            bool_tok!(tr.serif_headlines),
                "button_treatment":           int_tok!(tr.button_treatment),
                "invert_active_fill":         bool_tok!(tr.invert_active_fill),
                "vertical_group_dividers":    bool_tok!(tr.vertical_group_dividers),
                "show_active_tab_underline":  bool_tok!(tr.show_active_tab_underline),
                "inactive_header_fill":       bool_tok!(tr.inactive_header_fill),
                "nav_buttons_label_only":     bool_tok!(tr.nav_buttons_label_only),
                "nav_buttons_uppercase_labels": bool_tok!(tr.nav_buttons_uppercase_labels),
                "tab_underline_under_text":   bool_tok!(tr.tab_underline_under_text),
                "card_floating_shadow":       bool_tok!(tr.card_floating_shadow),
                "shadows_enabled":            bool_tok!(tr.shadows_enabled),
                "animations_enabled":         bool_tok!(tr.animations_enabled),
            },
            // DS-6.0: the shell block. Emitted as plain enum names rather than
            // DTCG token objects — these are structural choices, not values on
            // a scale, so there is nothing to interpolate or theme-shift.
            //
            // This MUST round-trip. A field that exports but does not import is
            // the "lossy pack round-trip" defect the architecture audit called
            // out; adding `shell` without this block broke three import tests
            // immediately, which is the system working.
            "shell": {
                "nav":       str_tok!(format!("{:?}", shl.nav)),
                "dock":      str_tok!(format!("{:?}", shl.dock)),
                "rail":      str_tok!(format!("{:?}", shl.rail)),
                "archetype": str_tok!(format!("{:?}", shl.archetype)),
            },
            "chrome": {
                "toolbar_height_scale":          num!(ch.toolbar_height_scale),
                "header_height_scale":           num!(ch.header_height_scale),
                "pane_header_compact_adjust": num!(ch.pane_header_compact_adjust),
                "account_strip_height":          dim!(ch.account_strip_height),
                "pane_border_width":             dim!(ch.pane_border_width),
                "pane_gap":                      dim!(ch.pane_gap),
                "pane_gap_alpha":                int_tok!(ch.pane_gap_alpha),
                "pane_active_indicator":         str_tok!(pane_indicator_str),
                "active_header_fill_multiply":   num!(ch.active_header_fill_multiply),
                "inactive_header_fill_multiply": num!(ch.inactive_header_fill_multiply),
                "header_outer_border_alpha":     int_tok!(ch.header_outer_border_alpha),
                "header_outer_border_width":     dim!(ch.header_outer_border_width),
                "header_divider_alpha":          int_tok!(ch.header_divider_alpha),
                "nav_active_col_alpha":          int_tok!(ch.nav_active_col_alpha),
                "dialog_backdrop_alpha":         int_tok!(ch.dialog_backdrop_alpha),
                "tab_inactive_alpha":            num!(ch.tab_inactive_alpha),
                "tab_hover_bg_alpha":            int_tok!(ch.tab_hover_bg_alpha),
                "tab_underline_thickness":       dim!(ch.tab_underline_thickness),
                "section_label_padding_top":     dim!(ch.section_label_padding_top),
                "section_label_padding_bottom":  dim!(ch.section_label_padding_bottom),
                "drag_handle_alpha":             num!(ch.drag_handle_alpha),
                "drag_handle_dot_scale":         num!(ch.drag_handle_dot_scale),
                "toast_bg_alpha":                int_tok!(ch.toast_bg_alpha),
                "card_stripe_alpha":             int_tok!(ch.card_stripe_alpha),
                "card_floating_shadow_alpha":    int_tok!(ch.card_floating_shadow_alpha),
                "accent_emphasis":               num!(ch.accent_emphasis),
                "disabled_opacity":              num!(ch.disabled_opacity),
                "focus_ring_width":              dim!(ch.focus_ring_width),
                "focus_ring_alpha":              int_tok!(ch.focus_ring_alpha),
                "hover_bg_alpha":                int_tok!(ch.hover_bg_alpha),
                "active_bg_alpha":               int_tok!(ch.active_bg_alpha),
                "region_gap":                    dim!(ch.region_gap),
                "region_radius":                 dim!(ch.region_radius),
                "region_border_alpha":           int_tok!(ch.region_border_alpha),
                "nav_cluster_radius":            dim!(ch.nav_cluster_radius),
                "nav_cluster_fill_alpha":        int_tok!(ch.nav_cluster_fill_alpha),
                "nav_cluster_padding":           dim!(ch.nav_cluster_padding),
                "button_group":                  str_tok!(group_enclosure_str),
                "toolnav_height":                dim!(ch.toolnav_height),
                "footer_default_open":           bool_tok!(ch.footer_default_open),
                "panel_header_treatment":        int_tok!(ch.panel_header_treatment),
                "panel_section_fill_alpha":      int_tok!(ch.panel_section_fill_alpha),
                "panel_footer_card":             bool_tok!(ch.panel_footer_card),
                "panel_footer_radius":           dim!(ch.panel_footer_radius),
            },
        });

        serde_json::to_string_pretty(&root).unwrap_or_default()
    }
}

// ── export_builtin_themes ────────────────────────────────────────────────────

/// Write every built-in [`ColorScheme`] and [`StyleSystem`] to disk in DTCG
/// JSON format.
///
/// Layout:
/// ```text
/// <dir>/
///   colorschemes/<id>.json   — one file per ColorScheme
///   styles/<id>.json         — one file per StyleSystem
/// ```
///
/// Both subdirectories are created if they do not already exist.
///
/// # Returns
///
/// The total number of files written (colorschemes + styles).
///
/// # Errors
///
/// Returns `Err` if a directory cannot be created or a file cannot be written.
/// Individual serialization failures are silently skipped (the count excludes
/// them), but `to_dtcg()` is infallible in practice.
pub fn export_builtin_themes(dir: &Path) -> io::Result<usize> {
    let cs_dir = dir.join("colorschemes");
    let st_dir = dir.join("styles");

    fs::create_dir_all(&cs_dir)?;
    fs::create_dir_all(&st_dir)?;

    let mut count = 0_usize;

    for scheme in builtin_color_schemes() {
        let path = cs_dir.join(format!("{}.json", scheme.meta.id));
        let json = scheme.to_dtcg();
        if !json.is_empty() {
            let mut f = fs::File::create(&path)?;
            f.write_all(json.as_bytes())?;
            count += 1;
        }
    }

    for style in builtin_style_systems() {
        let path = st_dir.join(format!("{}.json", style.meta.id));
        let json = style.to_dtcg();
        if !json.is_empty() {
            let mut f = fs::File::create(&path)?;
            f.write_all(json.as_bytes())?;
            count += 1;
        }
    }

    Ok(count)
}

// ── scan_theme_dir ───────────────────────────────────────────────────────────

/// Scan `<dir>/colorschemes/*.json` and `<dir>/styles/*.json`, parsing each
/// file via the DTCG loader.  Files that fail to parse are skipped with a
/// diagnostic printed to stderr — the function never panics.
///
///
pub fn scan_theme_dir(dir: &Path) -> (Vec<ColorScheme>, Vec<StyleSystem>) {
    let mut schemes = Vec::new();
    let mut styles  = Vec::new();

    // ── colorschemes ─────────────────────────────────────────────────────────
    let cs_dir = dir.join("colorschemes");
    if let Ok(rd) = fs::read_dir(&cs_dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path) {
                Err(e) => eprintln!("[design_system] cannot read {:?}: {e}", path),
                Ok(json) => match ColorScheme::from_dtcg(&json) {
                    Ok(cs) => schemes.push(cs),
                    Err(e) => eprintln!("[design_system] skipping {:?}: {}", path, format_load_error(&e)),
                },
            }
        }
    }

    // ── styles ───────────────────────────────────────────────────────────────
    let st_dir = dir.join("styles");
    if let Ok(rd) = fs::read_dir(&st_dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path) {
                Err(e) => eprintln!("[design_system] cannot read {:?}: {e}", path),
                Ok(json) => match StyleSystem::from_dtcg(&json) {
                    Ok(ss) => styles.push(ss),
                    Err(e) => eprintln!("[design_system] skipping {:?}: {}", path, format_load_error(&e)),
                },
            }
        }
    }

    (schemes, styles)
}

fn format_load_error(e: &LoadError) -> String {
    format!("{e}")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_system_to_dtcg_round_trip() {
        use crate::design_system::builtin::builtin_style_systems;
        let styles = builtin_style_systems();
        for original in &styles {
            let json = original.to_dtcg();
            assert!(!json.is_empty(), "to_dtcg() must not return empty string for '{}'", original.meta.id);
            let parsed = StyleSystem::from_dtcg(&json)
                .unwrap_or_else(|e| panic!("round-trip failed for '{}': {e}", original.meta.id));
            assert_eq!(parsed.meta.id, original.meta.id);
            assert_eq!(parsed.typography.size_sm, original.typography.size_sm);
            assert_eq!(parsed.radii.sm, original.radii.sm);
            assert_eq!(parsed.treatments.solid_active_fills, original.treatments.solid_active_fills);
            assert_eq!(parsed.treatments.focus_ring, original.treatments.focus_ring);
        }
    }

    #[test]
    fn export_and_scan_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let written = export_builtin_themes(dir).expect("export failed");
        // 22 colorschemes (16 THEMES + 5 React ports + Meridien) + 9 styles = 31
        assert_eq!(written, 31, "expected 31 files written (22 schemes + 9 styles)");

        let (schemes, styles) = scan_theme_dir(dir);
        assert_eq!(
            schemes.len(), 22,
            "scan_theme_dir must recover all 22 colorschemes, got {}",
            schemes.len()
        );
        assert_eq!(
            styles.len(), 9,
            "scan_theme_dir must recover all 9 style systems, got {}",
            styles.len()
        );
    }
}
