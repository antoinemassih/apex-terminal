//! Component recipes built on the `Sx` system — the cva (class-variance-authority)
//! proof. Each recipe is a `LazyLock` static, built once, then resolved per call.
//!
//! This is intentionally small: one `button` recipe demonstrating intent × size
//! variants resolving to composed `Sx`. Real component families would live here
//! (card, badge, input, …), each a `Recipe` + a thin render fn.

use std::sync::LazyLock;
use egui::{Response, RichText, Sense, Ui};

use super::{Recipe, Shade, StyleState, Sx, SxDelta, Tone};

type Theme = crate::chart_renderer::gpu::Theme;

/// `button` recipe — `intent` (primary/ghost/danger/success) × `size` (sm/md/lg).
/// Hover adds a composable accent ring that works regardless of intent fill.
pub static BUTTON: LazyLock<Recipe> = LazyLock::new(|| {
    Recipe::new(
        Sx::new()
            .rounded(6.0)
            .hover(|d| d.border_alpha(Tone::Accent, 130, 1.0)),
    )
    .variant("intent", "primary", &[
        ("primary", SxDelta::new().bg(Tone::Accent).text_shade(Tone::Bg, Shade::S50)),
        ("ghost",   SxDelta::new().border(Tone::Border, 1.0).text(Tone::Text)),
        ("danger",  SxDelta::new().bg(Tone::Bear).text_shade(Tone::Bg, Shade::S50)),
        ("success", SxDelta::new().bg(Tone::Bull).text_shade(Tone::Bg, Shade::S50)),
    ])
    .variant("size", "md", &[
        ("sm", SxDelta::new().px(8.0).py(3.0).text_size(11.0)),
        ("md", SxDelta::new().px(12.0).py(6.0).text_size(13.0)),
        ("lg", SxDelta::new().px(16.0).py(9.0).text_size(15.0)),
    ])
});

/// Render a recipe-driven button. Example:
/// ```ignore
/// if button(ui, t, "Buy", &[("intent","success"),("size","sm")]).clicked() { … }
/// ```
pub fn button(ui: &mut Ui, t: &Theme, label: &str, variants: &[(&str, &str)]) -> Response {
    let sx = BUTTON.resolve(variants);
    let size = sx.resolved(StyleState::Normal).text_size.unwrap_or(13.0);
    let txt = RichText::new(label).size(size); // color comes from sx's text override
    let (resp, _) = sx.show(ui, t, Sense::click(), |ui| {
        ui.add(egui::Label::new(txt).selectable(false));
    });
    resp
}
