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
        if s.monospace { rt = rt.monospace(); }
        if s.strong    { rt = rt.strong(); }
        rt
    }

    /// Convenience: emit a label using the default text color hint.
    pub fn apply(self, ui: &mut Ui, text: &str) -> Response {
        let color = ui.style().visuals.override_text_color
            .unwrap_or(TEXT_PRIMARY);
        ui.label(self.as_rich(text, color))
    }
}
