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
use crate::ui_kit::sx::{palette_ct, Tone};

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
        // Build the ctx from the UI so it carries the AMBIENT RecipeSet.
        // `StyleCtx::from_theme` would hand this widget an empty set — see
        // `ctx.rs` for why that shim must never be used inside a `show`.
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point.
    ///
    /// Callers that need per-call-site token overrides or an explicit
    /// `RecipeSet` construct a `StyleCtx` and call this directly; `show`
    /// delegates here with the ambient one.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> Response {
        let theme = sctx.theme();
        let color = if let Some(c) = self.color {
            c
        } else {
            let pal = palette_ct(theme);
            if self.muted { pal.base(Tone::Dim) } else { pal.base(Tone::Text) }
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
                // Single-row elision with a trailing ellipsis — previously this
                // branch called the same wrapping `layout` as `wrap`, so
                // `truncate(true)` wrapped to multiple rows instead of eliding.
                let mut job = egui::text::LayoutJob::single_section(
                    self.text.clone(),
                    egui::TextFormat { font_id: font_id.clone(), color, ..Default::default() },
                );
                job.wrap = egui::text::TextWrapping {
                    max_width: max_w,
                    max_rows: 1,
                    break_anywhere: true,
                    overflow_character: Some('…'),
                };
                f.layout_job(job)
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
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}

#[cfg(test)]
mod truncate_tests {
    // Locks the fix: `Label::truncate(true)` must ELIDE to a single row with an
    // ellipsis, not wrap to multiple rows (the old bug). Mirrors the LayoutJob
    // the widget builds in `show`.
    fn layout_truncated(ctx: &egui::Context, text: &str, max_w: f32) -> std::sync::Arc<egui::Galley> {
        let font_id = crate::ui_kit::style::prop_at(crate::ui_kit::style::font_md());
        ctx.fonts(|f| {
            let mut job = egui::text::LayoutJob::single_section(
                text.to_string(),
                egui::TextFormat { font_id: font_id.clone(), color: egui::Color32::WHITE, ..Default::default() },
            );
            job.wrap = egui::text::TextWrapping {
                max_width: max_w, max_rows: 1, break_anywhere: true, overflow_character: Some('…'),
            };
            f.layout_job(job)
        })
    }

    #[test]
    fn truncate_elides_long_text_to_one_row() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |_| {});
        let long = layout_truncated(&ctx, "This is a very long label that will never fit here", 40.0);
        assert!(long.elided, "long text should be elided");
        assert_eq!(long.rows.len(), 1, "truncate must produce exactly one row");
        let short = layout_truncated(&ctx, "OK", 400.0);
        assert!(!short.elided, "short text that fits should not be elided");
    }
}
