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
        match self {
            TextStyle::Display    => TextSpec { size: font_2xl() + 4.0,  strong: true,  monospace: false, line_height_factor: 1.25 },
            TextStyle::HeadingLg  => TextSpec { size: font_2xl(),        strong: true,  monospace: false, line_height_factor: 1.25 },
            TextStyle::HeadingMd  => TextSpec { size: font_xl(),         strong: true,  monospace: false, line_height_factor: 1.3  },
            TextStyle::BodyLg     => TextSpec { size: font_lg(),         strong: false, monospace: false, line_height_factor: 1.4  },
            TextStyle::Body       => TextSpec { size: st.font_body,      strong: false, monospace: false, line_height_factor: 1.4  },
            TextStyle::BodySm     => TextSpec { size: font_sm(),         strong: false, monospace: false, line_height_factor: 1.35 },
            TextStyle::Caption    => TextSpec { size: st.font_caption,   strong: false, monospace: false, line_height_factor: 1.3  },
            TextStyle::Mono       => TextSpec { size: st.font_body,      strong: false, monospace: true,  line_height_factor: 1.35 },
            TextStyle::MonoSm     => TextSpec { size: font_sm(),         strong: false, monospace: true,  line_height_factor: 1.3  },
            TextStyle::Numeric    => TextSpec { size: st.font_body,      strong: true,  monospace: true,  line_height_factor: 1.3  },
            TextStyle::NumericLg  => TextSpec { size: font_xl(),         strong: true,  monospace: true,  line_height_factor: 1.25 },
            TextStyle::NumericHero => TextSpec { size: 30.0,             strong: true,  monospace: true,  line_height_factor: 1.2  },
            TextStyle::Label      => TextSpec { size: st.font_section_label, strong: true,  monospace: false, line_height_factor: 1.3  },
            TextStyle::Eyebrow    => TextSpec { size: st.font_section_label, strong: true,  monospace: false, line_height_factor: 1.2  },
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
