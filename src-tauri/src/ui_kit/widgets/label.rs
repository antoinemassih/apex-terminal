//! Label — primary text widget. Wraps egui's text rendering with our
//! typography scale + theme tokens. Replaces ad-hoc `RichText` setups
//! and the SemanticLabel/MonospaceCode patterns scattered through panels.
//!
//! API:
//!   ui.add(Label::new("Total P/L").size(Size::Sm).muted());
//!   ui.add(Label::heading("Account Summary"));   // size Lg, semibold
//!   ui.add(Label::number("$12,345.67"));         // mono family

use egui::{Color32, FontFamily, FontId, Response, Sense, Ui, Widget};

use super::theme::ComponentTheme;
use super::tokens::Size;

#[derive(Clone)]
enum Family {
    Proportional,
    Monospace,
    Named(String),
}

#[must_use = "Label does nothing until `.show(ui, theme)` or `ui.add(label)` is called"]
pub struct Label<'a> {
    text: String,
    size: Size,
    family: Family,
    strong: bool,
    muted: bool,
    color: Option<Color32>,
    truncate: bool,
    wrap: bool,
    _lt: std::marker::PhantomData<&'a ()>,
}

impl<'a> Label<'a> {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            size: Size::Sm,
            family: Family::Proportional,
            strong: false,
            muted: false,
            color: None,
            truncate: false,
            wrap: true,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn heading(text: impl Into<String>) -> Self {
        let mut l = Self::new(text);
        l.size = Size::Lg;
        l.strong = true;
        l
    }

    pub fn subheading(text: impl Into<String>) -> Self {
        let mut l = Self::new(text);
        l.size = Size::Md;
        l.strong = true;
        l
    }

    pub fn number(text: impl Into<String>) -> Self {
        let mut l = Self::new(text);
        l.family = Family::Monospace;
        l
    }

    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    /// Override the font family by registered name (e.g. "inter_semibold",
    /// "inter_bold") — used by `PolishedLabel`'s fallback path to render
    /// a real heavier-weight face instead of egui's faux-bold.
    pub fn with_text_family(mut self, name: impl Into<String>) -> Self {
        self.family = Family::Named(name.into());
        self
    }
    pub fn muted(mut self) -> Self { self.muted = true; self }
    pub fn strong(mut self) -> Self { self.strong = true; self }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn truncate(mut self, v: bool) -> Self { self.truncate = v; if v { self.wrap = false; } self }
    pub fn wrap(mut self, v: bool) -> Self { self.wrap = v; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let color = if let Some(c) = self.color {
            c
        } else if self.muted {
            theme.dim()
        } else {
            theme.text()
        };
        let font_size = self.size.font_size();
        // Resolve family. `strong` upgrades the default Proportional
        // family to a real Inter SemiBold face (registered in
        // `init_fonts`) instead of egui's faux-bold stretch.
        let family = match &self.family {
            Family::Proportional => {
                if self.strong {
                    FontFamily::Name("inter_semibold".into())
                } else {
                    FontFamily::Proportional
                }
            }
            Family::Monospace => {
                if self.strong {
                    FontFamily::Name("jetbrains_mono_bold".into())
                } else {
                    FontFamily::Monospace
                }
            }
            Family::Named(n) => FontFamily::Name(n.clone().into()),
        };
        let font_id = FontId::new(font_size, family);

        let max_w = ui.available_width();
        let galley = ui.fonts(|f| {
            if self.truncate {
                f.layout(self.text.clone(), font_id.clone(), color, max_w)
            } else if self.wrap {
                f.layout(self.text.clone(), font_id.clone(), color, max_w)
            } else {
                f.layout_no_wrap(self.text.clone(), font_id.clone(), color)
            }
        });

        let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
        if ui.is_rect_visible(rect) {
            ui.painter().galley(rect.min, galley, color);
        }
        response
    }
}

impl<'a> Widget for Label<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}
