//! Foundation typography scale.
//!
//! Every shell that paints text routes through `TextStyle::as_rich(..)` so font
//! size / weight / monospace / line-height live in one place. Sizes come from
//! `style::font_*` helpers.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Ui};
use super::super::style::*;

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
    pub fn spec(self) -> TextSpec {
        // font_section_label / font_body / font_caption pull from StyleSettings so
        // the inspector knobs propagate to Label/Eyebrow, Body, and Caption roles.
        let st = current();
        // Line-height factors via named tokens (P2.5).
        use crate::ui_kit::style::{line_tight, line_heading, line_dense, line_compact, line_normal};
        match self {
            TextStyle::Display    => TextSpec { size: font_2xl() + 4.0,  strong: true,  monospace: false, line_height_factor: line_heading() },
            TextStyle::HeadingLg  => TextSpec { size: font_2xl(),        strong: true,  monospace: false, line_height_factor: line_heading() },
            TextStyle::HeadingMd  => TextSpec { size: font_xl(),         strong: true,  monospace: false, line_height_factor: line_dense()   },
            TextStyle::BodyLg     => TextSpec { size: font_lg(),         strong: false, monospace: false, line_height_factor: line_normal()  },
            TextStyle::Body       => TextSpec { size: st.font_body,      strong: false, monospace: false, line_height_factor: line_normal()  },
            TextStyle::BodySm     => TextSpec { size: font_sm(),         strong: false, monospace: false, line_height_factor: line_compact() },
            TextStyle::Caption    => TextSpec { size: st.font_caption,   strong: false, monospace: false, line_height_factor: line_dense()   },
            TextStyle::Mono       => TextSpec { size: st.font_body,      strong: false, monospace: true,  line_height_factor: line_compact() },
            TextStyle::MonoSm     => TextSpec { size: font_sm(),         strong: false, monospace: true,  line_height_factor: line_dense()   },
            TextStyle::Numeric    => TextSpec { size: st.font_body,      strong: true,  monospace: true,  line_height_factor: line_dense()   },
            TextStyle::NumericLg  => TextSpec { size: font_xl(),         strong: true,  monospace: true,  line_height_factor: line_heading() },
            TextStyle::NumericHero => TextSpec { size: font_display_sm() + 2.0, strong: true, monospace: true, line_height_factor: line_tight() },
            TextStyle::Label      => TextSpec { size: st.font_section_label, strong: true,  monospace: false, line_height_factor: line_dense() },
            TextStyle::Eyebrow    => TextSpec { size: st.font_section_label, strong: true,  monospace: false, line_height_factor: line_tight() },
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
    pub fn all() -> [TextStyle; 14] {
        [
            TextStyle::Display, TextStyle::HeadingLg, TextStyle::HeadingMd,
            TextStyle::BodyLg, TextStyle::Body, TextStyle::BodySm,
            TextStyle::Caption, TextStyle::Mono, TextStyle::MonoSm,
            TextStyle::Numeric, TextStyle::NumericLg, TextStyle::NumericHero,
            TextStyle::Label, TextStyle::Eyebrow,
        ]
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
    pub fn apply(self, ui: &mut Ui, text: &str) -> Response {
        let color = ui.style().visuals.override_text_color
            .unwrap_or_else(|| crate::chart_renderer::theme_impl::active_theme(ui.ctx()).text);
        ui.label(self.as_rich(text, color))
    }
}
