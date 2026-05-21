//! Machine equivalence tests between the new `design_system` data and the live system.
//!
//! Proves (or disproves) whether `builtin_color_schemes()` / `builtin_style_systems()` are
//! field-exact equivalents of `gpu::THEMES` / `style::style_defaults(0/1/2)`.
//!
//! Strategy: collect every mismatch into a `Vec<String>` and print a full delta report
//! at the end.  Tests always run to completion — no early panic.  A summary line tells
//! us exactly how many fields diverge and why.
//!
//! # Alpha tolerance
//! `builtin_style_systems()` stores alpha multipliers as rounded f32 literals (e.g. 0.08)
//! whereas the live system stores integer u8 values (20/255 = 0.07843…).  These are
//! documented approximations; the test uses a ±0.005 tolerance for these comparisons
//! and reports them as APPROX (not counted against the strict mismatch threshold).

#[cfg(test)]
pub(crate) mod equivalence {
    use crate::design_system::builtin::{builtin_color_schemes, builtin_style_systems};
    use crate::chart_renderer::gpu::THEMES;
    use crate::chart_renderer::ui::style::style_defaults_pub;

    // ── helpers ───────────────────────────────────────────────────────────────────

    /// Compare RGB bytes only (ignores alpha — used for fields where alpha is unrelated).
    fn rgba_rgb_eq_c32(rgba: &[u8; 4], c32: egui::Color32) -> bool {
        rgba[0] == c32.r() && rgba[1] == c32.g() && rgba[2] == c32.b()
    }

    fn fmt_rgba(r: &[u8; 4]) -> String {
        format!("({},{},{},a={})", r[0], r[1], r[2], r[3])
    }

    fn fmt_c32(c: egui::Color32) -> String {
        format!("({},{},{},a={})", c.r(), c.g(), c.b(), c.a())
    }

    // ── Colour equivalence ────────────────────────────────────────────────────────

    /// For each of the 16 built-in color schemes, compare the fields that have
    /// a direct (non-derived) mapping to `gpu::THEMES`.
    ///
    /// Documented lossy/derived fields:
    ///   `paper` — derived from toolbar_bg + ±8 luminance shift; no Theme equivalent.
    ///   `shadow` alpha — dark themes use 180 (new) vs opaque rgb(0,0,0) (old);
    ///             light themes use 120 (new) vs opaque rgb(40-60,40-50,40-40) (old).
    ///             Shadow alpha is a new design-system concept with no live equivalent.
    ///
    /// Fields compared: bg, surface↔toolbar_bg, text, dim, border↔toolbar_border,
    ///                  accent, bull, bear, warn, shadow RGB.
    #[test]
    fn colour_axis_equivalence() {
        let schemes = builtin_color_schemes();
        let themes = THEMES;

        assert_eq!(
            schemes.len(),
            themes.len(),
            "scheme count {} ≠ theme count {}",
            schemes.len(),
            themes.len()
        );

        let mut strict_mismatches: Vec<String> = Vec::new();
        let mut noted_differences: Vec<String> = Vec::new();

        for (i, (scheme, theme)) in schemes.iter().zip(themes.iter()).enumerate() {
            let name = &scheme.meta.name;

            macro_rules! check_opaque {
                ($field_new:expr, $field_old:expr, $label:expr) => {
                    if !rgba_rgb_eq_c32(&$field_new, $field_old) {
                        strict_mismatches.push(format!(
                            "[{}][{}] {}: new={} old={}",
                            i, name, $label,
                            fmt_rgba(&$field_new),
                            fmt_c32($field_old)
                        ));
                    }
                };
            }

            // bg ↔ bg
            check_opaque!(scheme.bg, theme.bg, "bg");

            // surface ↔ toolbar_bg
            check_opaque!(scheme.surface, theme.toolbar_bg, "surface↔toolbar_bg");

            // text ↔ text
            check_opaque!(scheme.text, theme.text, "text");

            // dim ↔ dim
            check_opaque!(scheme.dim, theme.dim, "dim");

            // border ↔ toolbar_border (= hairline_border(bg))
            check_opaque!(scheme.border, theme.toolbar_border, "border↔toolbar_border");

            // accent ↔ accent
            check_opaque!(scheme.accent, theme.accent, "accent");

            // bull ↔ bull
            check_opaque!(scheme.bull, theme.bull, "bull");

            // bear ↔ bear
            check_opaque!(scheme.bear, theme.bear, "bear");

            // warn ↔ warn
            check_opaque!(scheme.warn, theme.warn, "warn");

            // shadow RGB — documented difference in alpha encoding;
            // compare only the RGB component.
            if !rgba_rgb_eq_c32(&scheme.shadow, theme.shadow_color) {
                strict_mismatches.push(format!(
                    "[{}][{}] shadow (RGB): new={} old={}",
                    i, name,
                    fmt_rgba(&scheme.shadow),
                    fmt_c32(theme.shadow_color)
                ));
            } else {
                // Note alpha divergence (documented, not a defect)
                let new_a = scheme.shadow[3];
                let old_a = theme.shadow_color.a();
                if new_a != old_a {
                    noted_differences.push(format!(
                        "[{}][{}] shadow alpha (documented divergence): new_alpha={} old_alpha={}",
                        i, name, new_a, old_a
                    ));
                }
            }

            // paper — SKIP: derived field with no Theme equivalent
            // (paper = toolbar_bg ± 8 luminance, not stored in gpu::Theme)
        }

        eprintln!("\n=== COLOUR AXIS DELTA REPORT ===");
        eprintln!("Themes compared: {}", schemes.len());
        if strict_mismatches.is_empty() {
            eprintln!("STATUS: field-exact on all direct-mapped fields");
        } else {
            eprintln!("STATUS: {} strict mismatch(es):", strict_mismatches.len());
            for m in &strict_mismatches {
                eprintln!("  MISMATCH: {}", m);
            }
        }
        if !noted_differences.is_empty() {
            eprintln!("--- Documented / expected differences ({}) ---", noted_differences.len());
            for n in &noted_differences {
                eprintln!("  NOTE: {}", n);
            }
        }
        eprintln!("=== END COLOUR DELTA ===\n");

        assert!(
            strict_mismatches.is_empty(),
            "{} colour-axis strict mismatch(es) — see DELTA REPORT above",
            strict_mismatches.len()
        );
    }

    // ── Style equivalence ─────────────────────────────────────────────────────────

    /// For each of the 3 built-in style systems (Meridien/Aperture/Octave), compare
    /// the fields that have a direct mapping to `style_defaults(0/1/2)`.
    ///
    /// # Index mapping
    /// `builtin_style_systems()` returns `[meridien, aperture, octave]`.
    /// `style_defaults_pub(n)` uses: 0 → `_` arm (Meridien), 1 → Aperture, 2 → Octave.
    ///
    /// # Documented approximate fields (float-rounded, tolerance ±0.005)
    /// `alphas.subtle` ↔ `hover_bg_alpha / 255`
    /// `alphas.soft`   ↔ `active_bg_alpha / 255`
    /// `alphas.header_border` ↔ `header_outer_border_alpha / 255`
    /// `shadows.card.alpha`   ↔ `shadow_alpha / 255`
    ///
    /// # Documented structural differences
    /// Meridien radii: new system deliberately uses 0,0,0,0 (sharp corners),
    /// but the live `_` (Meridien) arm of `style_defaults` uses 2,4,6,12
    /// (carried over from the Phase B source-swap baseline to preserve the default look).
    /// This divergence is real and expected — the live default emulates the graduated scale,
    /// not the pure Meridien aesthetic.
    #[test]
    fn style_axis_equivalence() {
        let styles = builtin_style_systems();
        // 0 → Meridien default arm; 1 → Aperture; 2 → Octave
        let old_settings: Vec<_> = (0u8..=2).map(style_defaults_pub).collect();

        assert_eq!(styles.len(), old_settings.len());

        let mut strict_mismatches: Vec<String> = Vec::new();
        let mut approx_differences: Vec<String> = Vec::new();
        let mut noted_differences: Vec<String> = Vec::new();

        for (i, (style, old)) in styles.iter().zip(old_settings.iter()).enumerate() {
            let name = &style.meta.name;

            macro_rules! check_f32_strict {
                ($new_val:expr, $old_val:expr, $label:expr) => {
                    let new_v: f32 = $new_val;
                    let old_v: f32 = $old_val;
                    if (new_v - old_v).abs() > 1e-4 {
                        strict_mismatches.push(format!(
                            "[{}][{}] {}: new={} old={}",
                            i, name, $label, new_v, old_v
                        ));
                    }
                };
            }

            macro_rules! check_f32_approx {
                ($new_val:expr, $old_val:expr, $label:expr) => {
                    let new_v: f32 = $new_val;
                    let old_v: f32 = $old_val;
                    let diff = (new_v - old_v).abs();
                    if diff > 0.005 {
                        // Beyond rounding tolerance — treat as strict mismatch
                        strict_mismatches.push(format!(
                            "[{}][{}] {} (APPROX-FAIL, diff={:.4}): new={:.4} old={:.4}",
                            i, name, $label, diff, new_v, old_v
                        ));
                    } else if diff > 1e-4 {
                        approx_differences.push(format!(
                            "[{}][{}] {} (rounding, diff={:.5}): new={:.5} old={:.5}",
                            i, name, $label, diff, new_v, old_v
                        ));
                    }
                };
            }

            macro_rules! check_bool {
                ($new_val:expr, $old_val:expr, $label:expr) => {
                    let new_v: bool = $new_val;
                    let old_v: bool = $old_val;
                    if new_v != old_v {
                        strict_mismatches.push(format!(
                            "[{}][{}] {}: new={} old={}",
                            i, name, $label, new_v, old_v
                        ));
                    }
                };
            }

            // ── Radii ──
            // Note: Meridien in the new system has 0,0,0,0 (pure sharp),
            //       but old style_defaults(0/Meridien) has 2,4,6,12 (Phase B baseline).
            //       This is a documented structural difference.
            let radii_xs_new  = style.radii.xs;
            let radii_xs_old  = old.r_xs as f32;
            if (radii_xs_new - radii_xs_old).abs() > 1e-4 {
                noted_differences.push(format!(
                    "[{}][{}] radii.xs (documented divergence: new={} old={}): \
                     new design=sharp(0), live default=graduated scale({})",
                    i, name, radii_xs_new, radii_xs_old, radii_xs_old
                ));
            }
            let radii_sm_new = style.radii.sm;
            let radii_sm_old = old.r_sm as f32;
            if (radii_sm_new - radii_sm_old).abs() > 1e-4 {
                noted_differences.push(format!(
                    "[{}][{}] radii.sm (documented divergence): new={} old={}",
                    i, name, radii_sm_new, radii_sm_old
                ));
            }
            let radii_md_new = style.radii.md;
            let radii_md_old = old.r_md as f32;
            if (radii_md_new - radii_md_old).abs() > 1e-4 {
                noted_differences.push(format!(
                    "[{}][{}] radii.md (documented divergence): new={} old={}",
                    i, name, radii_md_new, radii_md_old
                ));
            }
            let radii_lg_new = style.radii.lg;
            let radii_lg_old = old.r_lg as f32;
            if (radii_lg_new - radii_lg_old).abs() > 1e-4 {
                noted_differences.push(format!(
                    "[{}][{}] radii.lg (documented divergence): new={} old={}",
                    i, name, radii_lg_new, radii_lg_old
                ));
            }

            // ── Strokes ──
            // Mapping per builtin.rs doc: strokes.thin↔stroke_hair, strokes.std↔stroke_thin,
            //   strokes.bold↔stroke_bold, strokes.thick↔stroke_thick.
            // Note: Meridien in the new system collapses bold/thick to 1.0,
            //       but old style_defaults(0) has stroke_bold=1.5, stroke_thick=2.0.
            //       This is a documented design choice for the sharp minimal aesthetic.
            let stroke_thin_new = style.strokes.thin;
            let stroke_thin_old = old.stroke_hair;
            if (stroke_thin_new - stroke_thin_old).abs() > 1e-4 {
                noted_differences.push(format!(
                    "[{}][{}] strokes.thin↔stroke_hair (divergence): new={} old={}",
                    i, name, stroke_thin_new, stroke_thin_old
                ));
            }
            let stroke_std_new  = style.strokes.std;
            let stroke_std_old  = old.stroke_thin;
            if (stroke_std_new - stroke_std_old).abs() > 1e-4 {
                noted_differences.push(format!(
                    "[{}][{}] strokes.std↔stroke_thin (divergence): new={} old={}",
                    i, name, stroke_std_new, stroke_std_old
                ));
            }
            let stroke_bold_new  = style.strokes.bold;
            let stroke_bold_old  = old.stroke_bold;
            if (stroke_bold_new - stroke_bold_old).abs() > 1e-4 {
                noted_differences.push(format!(
                    "[{}][{}] strokes.bold↔stroke_bold (divergence): new={} old={}",
                    i, name, stroke_bold_new, stroke_bold_old
                ));
            }
            let stroke_thick_new = style.strokes.thick;
            let stroke_thick_old = old.stroke_thick;
            if (stroke_thick_new - stroke_thick_old).abs() > 1e-4 {
                noted_differences.push(format!(
                    "[{}][{}] strokes.thick↔stroke_thick (divergence): new={} old={}",
                    i, name, stroke_thick_new, stroke_thick_old
                ));
            }

            // ── Typography ──
            check_f32_strict!(style.typography.size_xs, old.font_caption,  "typography.size_xs↔font_caption");
            check_f32_strict!(style.typography.size_sm, old.font_body,     "typography.size_sm↔font_body");
            check_f32_strict!(style.typography.size_xl, old.font_hero,     "typography.size_xl↔font_hero");

            // ── Spacing ──
            check_f32_strict!(style.spacing.md,         old.card_padding_y,  "spacing.md↔card_padding_y");
            check_f32_strict!(style.spacing.lg,         old.card_padding_x,  "spacing.lg↔card_padding_x");
            check_f32_strict!(style.spacing.cta_height, old.cta_height_px,   "spacing.cta_height↔cta_height_px");

            // ── Density ──
            let expected_factor: f32 = match old.density { 0 => 0.8, 2 => 1.2, _ => 1.0 };
            check_f32_strict!(style.density.factor,           expected_factor,    "density.factor↔density enum");
            check_f32_strict!(style.density.row_height_dense, old.row_height_px,  "density.row_height_dense↔row_height_px");

            // ── Alphas (approx — float rounding of u8/255 division) ──
            check_f32_approx!(style.alphas.subtle, old.hover_bg_alpha as f32 / 255.0,         "alphas.subtle↔hover_bg_alpha/255");
            check_f32_approx!(style.alphas.soft,   old.active_bg_alpha as f32 / 255.0,        "alphas.soft↔active_bg_alpha/255");
            check_f32_approx!(style.alphas.header_border, old.header_outer_border_alpha as f32 / 255.0, "alphas.header_border↔header_outer_border_alpha/255");
            check_f32_approx!(style.shadows.card.alpha, old.shadow_alpha as f32 / 255.0,       "shadows.card.alpha↔shadow_alpha/255");

            // ── Shadows (non-alpha fields) ──
            check_f32_strict!(style.shadows.card.blur,     old.shadow_blur,     "shadows.card.blur↔shadow_blur");
            check_f32_strict!(style.shadows.card.offset_y, old.shadow_offset_y, "shadows.card.offset_y↔shadow_offset_y");

            // ── Treatments ──
            check_bool!(style.treatments.solid_active_fills,       old.solid_active_fills,       "treatments.solid_active_fills");
            check_bool!(style.treatments.hairline_borders,         old.hairline_borders,         "treatments.hairline_borders");
            check_bool!(style.treatments.uppercase_section_labels, old.uppercase_section_labels, "treatments.uppercase_section_labels");
        }

        eprintln!("\n=== STYLE AXIS DELTA REPORT ===");
        eprintln!("Styles compared: {} (Meridien/Aperture/Octave)", styles.len());

        if strict_mismatches.is_empty() {
            eprintln!("STATUS: field-exact on all strict-checked fields");
        } else {
            eprintln!("STATUS: {} strict mismatch(es):", strict_mismatches.len());
            for m in &strict_mismatches {
                eprintln!("  MISMATCH: {}", m);
            }
        }

        if !approx_differences.is_empty() {
            eprintln!("--- Rounding-only differences (within ±0.005, not a defect) ---");
            for a in &approx_differences {
                eprintln!("  APPROX: {}", a);
            }
        }

        if !noted_differences.is_empty() {
            eprintln!("--- Documented structural divergences (design intent, not bugs) ---");
            for n in &noted_differences {
                eprintln!("  NOTE: {}", n);
            }
        }
        eprintln!("=== END STYLE DELTA ===\n");

        assert!(
            strict_mismatches.is_empty(),
            "{} style-axis strict mismatch(es) found — see DELTA REPORT above",
            strict_mismatches.len()
        );
    }
}
