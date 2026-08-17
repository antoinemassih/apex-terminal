//! Foundation typography scale.
//!
//! Every shell that paints text routes through `TextStyle::as_rich(..)` so font
//! size / weight / monospace / line-height live in one place. Sizes come from
//! `style::font_*` helpers.


use egui::{Color32, Response, RichText, Ui};
use crate::ui_kit::style::{
    font_xs, font_sm, font_md, font_lg, font_xl, font_2xl, font_display_sm,
    font_body, font_caption, font_section_label,
    line_tight, line_heading, line_dense, line_compact, line_normal,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyle {
    Display,
    HeadingLg,
    HeadingMd,
    BodyLg,
    Body,
    BodySm,
    Caption,
    Mono,
    MonoSm,
    /// Smallest mono rung (font_xs). Added to make the mono ladder monotonic:
    /// MonoXs(10) < MonoSm(12) < MonoMd(14).
    MonoXs,
    /// 14px mono — the TABULAR DATA rung. Matches the watchlist/DOM price
    /// ladder, which previously had no tier within 2-3px and therefore could
    /// not join the cascade at all. Distinct from `Numeric`, which stays tied
    /// to the per-style `font_body` for the surfaces already tuned around it.
    MonoMd,
    Numeric,
    NumericLg,
    NumericHero,
    Label,
    Eyebrow,
}

#[derive(Clone, Copy, Debug)]
pub struct TextSpec {
    pub size: f32,
    pub strong: bool,
    pub monospace: bool,
    /// Multiplier applied to size to derive line-height.
    pub line_height_factor: f32,
}

impl TextStyle {
    /// M1 Change D: whether the numeric display tiers render MONO. Classic =
    /// true; a style authoring `numerals.family == Ui/Display` flips the hero
    /// numerals to proportional (Aperture's Inter Tight 500 look).
    pub fn numeric_display_is_mono() -> bool {
        use crate::design_system::style_system::FontRole;
        match crate::ui_kit::style::numeral_tier() {
            Some(nt) => nt.family == FontRole::Mono,
            None => true,
        }
    }

    pub fn spec(self) -> TextSpec {
        // font_section_label / font_body / font_caption pull from StyleSettings so
        // the inspector knobs propagate to Label/Eyebrow, Body, and Caption roles.
        // Line-height factors via named tokens (P2.5).
                match self {
            TextStyle::Display    => TextSpec { size: font_2xl() + 4.0,  strong: true,  monospace: false, line_height_factor: line_heading() },
            TextStyle::HeadingLg  => TextSpec { size: font_2xl(),        strong: true,  monospace: false, line_height_factor: line_heading() },
            TextStyle::HeadingMd  => TextSpec { size: font_xl(),         strong: true,  monospace: false, line_height_factor: line_dense()   },
            TextStyle::BodyLg     => TextSpec { size: font_lg(),         strong: false, monospace: false, line_height_factor: line_normal()  },
            TextStyle::Body       => TextSpec { size: font_body(),      strong: false, monospace: false, line_height_factor: line_normal()  },
            TextStyle::BodySm     => TextSpec { size: font_sm(),         strong: false, monospace: false, line_height_factor: line_compact() },
            TextStyle::Caption    => TextSpec { size: font_caption(),   strong: false, monospace: false, line_height_factor: line_dense()   },
            TextStyle::Mono       => TextSpec { size: font_body(),      strong: false, monospace: true,  line_height_factor: line_compact() },
            TextStyle::MonoSm     => TextSpec { size: font_sm(),         strong: false, monospace: true,  line_height_factor: line_dense()   },
            TextStyle::MonoXs     => TextSpec { size: font_xs(),         strong: false, monospace: true,  line_height_factor: line_dense()   },
            TextStyle::MonoMd     => TextSpec { size: font_md(),         strong: false, monospace: true,  line_height_factor: line_dense()   },
            TextStyle::Numeric    => TextSpec { size: font_body(),      strong: true,  monospace: true,  line_height_factor: line_dense()   },
            TextStyle::NumericLg  => TextSpec { size: font_xl(),         strong: true,  monospace: true,  line_height_factor: line_heading() },
            TextStyle::NumericHero => TextSpec { size: font_display_sm() + 2.0, strong: true, monospace: true, line_height_factor: line_tight() },
            TextStyle::Label      => TextSpec { size: font_section_label(), strong: true,  monospace: false, line_height_factor: line_dense() },
            TextStyle::Eyebrow    => TextSpec { size: font_section_label(), strong: true,  monospace: false, line_height_factor: line_tight() },
        }
    }

    /// Build a `RichText` with this style applied (color provided by caller).
    pub fn as_rich(self, text: &str, color: Color32) -> RichText {
        let s = self.spec();
        let mut rt = RichText::new(text).size(s.size).color(color);
        // Apply the tier's line height. `line_height_factor` was computed for
        // all 14 tiers and then NEVER read — the whole vertical-rhythm half of
        // the type system was dead code, which is why line spacing felt
        // arbitrary even in files that adopted TextStyle. Now wired.
        if s.line_height_factor > 0.0 {
            rt = rt.line_height(Some(s.size * s.line_height_factor));
        }
        if s.monospace { rt = rt.monospace(); }
        if s.strong    { rt = rt.strong(); }
        rt
    }

    // ── egui text_styles CASCADE ────────────────────────────────────────────
    //
    // egui's `Style::text_styles` is a semantic-name → FontId table that child
    // `Ui`s INHERIT (egui clones the parent's Arc<Style>). That is a real CSS-
    // style cascade, and this app never used it: every one of ~626 text sites
    // re-specified its own size, which is exactly how 70% of the UI drifted onto
    // 9-11px. Registering the tiers here means:
    //   * one table defines the scale (edit it once, the app follows), and
    //   * any subtree can override a tier for its children —
    //     `ui.style_mut().text_styles.insert(TextStyle::Body.egui(), smaller)` —
    //     which is the thing hand-passed `FontId`s can never do.

    /// Stable name for this tier in egui's `text_styles` table.
    pub fn egui_name(self) -> &'static str {
        match self {
            TextStyle::Display => "apex.Display",
            TextStyle::HeadingLg => "apex.HeadingLg",
            TextStyle::HeadingMd => "apex.HeadingMd",
            TextStyle::BodyLg => "apex.BodyLg",
            TextStyle::Body => "apex.Body",
            TextStyle::BodySm => "apex.BodySm",
            TextStyle::Caption => "apex.Caption",
            TextStyle::Mono => "apex.Mono",
            TextStyle::MonoSm => "apex.MonoSm",
            TextStyle::MonoXs => "apex.MonoXs",
            TextStyle::MonoMd => "apex.MonoMd",
            TextStyle::Numeric => "apex.Numeric",
            TextStyle::NumericLg => "apex.NumericLg",
            TextStyle::NumericHero => "apex.NumericHero",
            TextStyle::Label => "apex.Label",
            TextStyle::Eyebrow => "apex.Eyebrow",
        }
    }

    /// This tier as an `egui::TextStyle` key (for `RichText::text_style`).
    pub fn egui(self) -> egui::TextStyle {
        egui::TextStyle::Name(self.egui_name().into())
    }

    /// Every tier — the single list `install` and tests iterate.
    pub fn all() -> [TextStyle; 16] {
        [
            TextStyle::Display, TextStyle::HeadingLg, TextStyle::HeadingMd,
            TextStyle::BodyLg, TextStyle::Body, TextStyle::BodySm,
            TextStyle::Caption, TextStyle::Mono, TextStyle::MonoSm,
            TextStyle::MonoXs, TextStyle::MonoMd,
            TextStyle::Numeric, TextStyle::NumericLg, TextStyle::NumericHero,
            TextStyle::Label, TextStyle::Eyebrow,
        ]
    }

    /// The `FontId` for this tier **resolved through the inherited style** —
    /// the cascade-aware form of [`Self::font_id`].
    ///
    /// `RichText::text_style()` only cascades for widget text. A huge amount of
    /// this app paints directly (`painter.text(.., FontId, ..)`), and those
    /// sites cannot use `text_style` at all — which is why they all ended up
    /// hardcoding sizes. This reads the tier out of `ui.style().text_styles`,
    /// so a subtree override
    /// (`ui.style_mut().text_styles.insert(TextStyle::Body.egui(), f)`)
    /// reaches painter-drawn text too. Falls back to the global spec when the
    /// host never called [`Self::install`] (portable/headless hosts).
    pub fn font_id_in(self, ui: &Ui) -> egui::FontId {
        ui.style()
            .text_styles
            .get(&self.egui())
            .cloned()
            .unwrap_or_else(|| self.font_id())
    }

    /// This tier's FAMILY at an explicit size — for subtree cascade overrides
    /// (`style.text_styles.insert(tier.egui(), tier.font_id_at(sz))`).
    ///
    /// The tier owns whether it is mono or proportional, so callers re-pointing
    /// a tier for a subtree never construct a `FontId` themselves and cannot
    /// accidentally flip the family.
    pub fn font_id_at(self, size: f32) -> egui::FontId {
        if self.spec().monospace {
            egui::FontId::monospace(size)
        } else {
            egui::FontId::proportional(size)
        }
    }

    /// The `FontId` this tier resolves to right now (per-style tokens applied).
    pub fn font_id(self) -> egui::FontId {
        let s = self.spec();
        if s.monospace {
            egui::FontId::monospace(s.size)
        } else {
            egui::FontId::proportional(s.size)
        }
    }

    /// Register all 14 tiers into an `egui::Style`'s inherited `text_styles`
    /// table. Called once per frame from `setup_theme` so the tiers track the
    /// active StyleSystem (font_body/font_caption/font_section_label are
    /// per-style) and any live token edit.
    pub fn install(style: &mut egui::Style) {
        for tier in Self::all() {
            style.text_styles.insert(tier.egui(), tier.font_id());
        }
    }

    /// Build `RichText` that reads its size from the INHERITED table rather
    /// than baking one in — the cascading counterpart to [`Self::as_rich`].
    /// Prefer this at new call sites; `as_rich` stays for the sites that have
    /// not been migrated (and for hosts that never called `install`).
    pub fn as_rich_cascading(self, text: &str, color: Color32) -> RichText {
        let s = self.spec();
        let mut rt = RichText::new(text).text_style(self.egui()).color(color);
        if s.line_height_factor > 0.0 {
            rt = rt.line_height(Some(s.size * s.line_height_factor));
        }
        if s.strong { rt = rt.strong(); }
        rt
    }

    /// Convenience: emit a label using the default text color hint.
    ///
    /// M2.1: was `chart_renderer::theme_impl::active_theme(ctx).text` — a
    /// chart-layer reach the layer guard rightly rejects now that this file
    /// lives in ui_kit. The portable ambient theme carries the same `text`.
    pub fn apply(self, ui: &mut Ui, text: &str) -> Response {
        let color = ui.style().visuals.override_text_color
            .unwrap_or_else(|| crate::ui_kit::widgets::theme::active_theme(ui.ctx()).text);
        ui.label(self.as_rich(text, color))
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod cascade_tests {
    use super::*;

    /// `install()` iterates `all()`, and `RichText::text_style()` resolves via
    /// `egui::TextStyle::resolve`, which **panics** on a Name key that is not in
    /// the table. So a variant missing from `all()` is not a cosmetic slip — it
    /// is a runtime panic the moment anything renders that tier. (This actually
    /// happened while adding MonoXs/MonoMd.) Enumerate exhaustively so the
    /// compiler forces this list to stay complete.
    #[test]
    fn all_contains_every_variant() {
        let all = TextStyle::all();
        for tier in [
            TextStyle::Display, TextStyle::HeadingLg, TextStyle::HeadingMd,
            TextStyle::BodyLg, TextStyle::Body, TextStyle::BodySm,
            TextStyle::Caption, TextStyle::Mono, TextStyle::MonoSm,
            TextStyle::MonoXs, TextStyle::MonoMd, TextStyle::Numeric,
            TextStyle::NumericLg, TextStyle::NumericHero, TextStyle::Label,
            TextStyle::Eyebrow,
        ] {
            assert!(all.contains(&tier), "{tier:?} missing from TextStyle::all() — install() would skip it and text_style() would panic on resolve");
        }
    }

    /// Names must be unique, or two tiers collide in the table and one silently
    /// wins.
    #[test]
    fn egui_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for tier in TextStyle::all() {
            assert!(seen.insert(tier.egui_name()), "duplicate egui_name: {}", tier.egui_name());
        }
    }

    /// The mono ladder must be strictly ordered. It was NOT: `Mono` resolved to
    /// the per-style `font_body` (10-13) while `MonoSm` was the fixed 12px
    /// token, so "Sm" rendered LARGER than the base tier under the default
    /// style, and the ordering flipped between StyleSystems. Both migration
    /// agents independently tripped over this.
    #[test]
    fn mono_ladder_is_monotonic() {
        let xs = TextStyle::MonoXs.spec().size;
        let sm = TextStyle::MonoSm.spec().size;
        let md = TextStyle::MonoMd.spec().size;
        assert!(xs < sm, "MonoXs ({xs}) must be smaller than MonoSm ({sm})");
        assert!(sm < md, "MonoSm ({sm}) must be smaller than MonoMd ({md})");
    }

    /// Heading tiers must descend. `font_2xl` was once aliased to `font_lg`
    /// (16), which is SMALLER than `font_xl` (22) — that inverted the ladder so
    /// switching HeadingMd -> HeadingLg made text shrink.
    #[test]
    fn heading_ladder_descends() {
        let d = TextStyle::Display.spec().size;
        let lg = TextStyle::HeadingLg.spec().size;
        let md = TextStyle::HeadingMd.spec().size;
        assert!(d > lg, "Display ({d}) must exceed HeadingLg ({lg})");
        assert!(lg > md, "HeadingLg ({lg}) must exceed HeadingMd ({md})");
    }

    /// Every tier must produce a usable size — a 0.0 renders invisibly.
    #[test]
    fn every_tier_has_a_sane_size() {
        for tier in TextStyle::all() {
            let s = tier.spec().size;
            assert!(s >= 6.0 && s <= 80.0, "{tier:?} size {s} out of range");
        }
    }
}
