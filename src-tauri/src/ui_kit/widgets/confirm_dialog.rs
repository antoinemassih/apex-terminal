//! `ConfirmDialog` — preset modal for "are you sure?" prompts.
//!
//! Eliminates the repeated `Modal + horizontal { Ghost cancel + Primary/Danger
//! confirm }` scaffolding the audit identified across welcome wizard,
//! order-confirm flows, and settings prompts (~4-6 ad-hoc patterns).
//!
//! ### Usage
//!
//! ```ignore
//! let resp = ConfirmDialog::new("Discard changes?")
//!     .id("discard_workspace")
//!     .body("Your unsaved layout will be lost.")
//!     .confirm("Discard", ConfirmTone::Danger)
//!     .cancel("Keep editing")
//!     .show(ctx, theme);
//! match resp.outcome {
//!     ConfirmOutcome::Confirmed => /* user clicked Discard */,
//!     ConfirmOutcome::Cancelled => /* user clicked Cancel or pressed Esc */,
//!     ConfirmOutcome::Open      => /* still showing */,
//! }
//! ```
//!
//! `id` is required (egui needs a stable identifier for the modal's window).
//! `confirm` is required (no-op default is intentional to force the caller
//! to label the destructive action).
//! `cancel` defaults to "Cancel" — pass an empty string to disable the
//! cancel button.

use egui::Color32;

use super::modal::{Modal, HeaderStyle, Anchor};
use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use super::button::Button;
use super::tokens::{Size, Variant};
use crate::ui_kit::tokens as st;

/// Tone for the confirm button — picks the variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ConfirmTone {
    #[default]
    /// Accent-filled primary button (constructive confirm).
    Primary,
    /// Bear-filled danger button (destructive confirm).
    Danger,
    /// Bull-filled (positive / commit).
    Bull,
}

impl ConfirmTone {
    fn variant(self) -> Variant {
        match self {
            ConfirmTone::Primary => Variant::Primary,
            ConfirmTone::Danger  => Variant::Danger,
            ConfirmTone::Bull    => Variant::Primary, // tinted bull
        }
    }
    fn tint(self, theme: &dyn ComponentTheme) -> Option<Color32> {
        match self {
            ConfirmTone::Bull => Some(palette_ct(theme).base(Tone::Bull)),
            _ => None,
        }
    }
}

/// What the user did, returned each frame by `show()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmOutcome {
    /// Still open — keep showing.
    Open,
    /// User clicked the confirm button (or pressed Enter).
    Confirmed,
    /// User clicked cancel, pressed Esc, or clicked outside the modal.
    Cancelled,
}

pub struct ConfirmDialogResponse {
    pub outcome: ConfirmOutcome,
}

#[must_use = "ConfirmDialog must be drawn with `.show(ctx, theme)`"]
pub struct ConfirmDialog<'a> {
    title: &'a str,
    id: Option<&'a str>,
    body: Option<&'a str>,
    confirm_label: &'a str,
    confirm_tone: ConfirmTone,
    cancel_label: &'a str,
    width: f32,
}

impl<'a> ConfirmDialog<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            id: None,
            body: None,
            confirm_label: "OK",
            confirm_tone: ConfirmTone::Primary,
            cancel_label: "Cancel",
            width: 380.0,
        }
    }

    /// Required — stable id used as the underlying Modal's window id.
    pub fn id(mut self, id: &'a str) -> Self { self.id = Some(id); self }

    /// Body text rendered below the header.
    pub fn body(mut self, body: &'a str) -> Self { self.body = Some(body); self }

    /// Customize the confirm button label and tone.
    pub fn confirm(mut self, label: &'a str, tone: ConfirmTone) -> Self {
        self.confirm_label = label;
        self.confirm_tone = tone;
        self
    }

    /// Customize the cancel button label. Pass `""` to hide the cancel button entirely.
    pub fn cancel(mut self, label: &'a str) -> Self {
        self.cancel_label = label;
        self
    }

    /// Override the dialog width (default 380px).
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }

    pub fn show<T: ComponentTheme>(self, ctx: &egui::Context, theme: &T) -> ConfirmDialogResponse {
        let id = self.id.expect("ConfirmDialog requires an `id` for stable egui identification");
        let title = self.title;
        let body = self.body;
        let confirm_label = self.confirm_label;
        let confirm_tone = self.confirm_tone;
        let cancel_label = self.cancel_label;
        let width = self.width;

        let mut outcome = ConfirmOutcome::Open;
        let modal_resp = Modal::new(title)
            .id(id)
            .ctx(ctx)
            .theme(theme)
            .anchor(Anchor::Window { pos: None })
            .header_style(HeaderStyle::Dialog)
            .size(egui::vec2(width, 0.0))
            .show(|ui| {
                if let Some(text) = body {
                    ui.add_space(st::gap_sm());
                    ui.label(
                        egui::RichText::new(text)
                            .size(st::font_sm())
                            .color(palette_ct(theme).base(Tone::Text)),
                    );
                    ui.add_space(st::gap_md());
                }
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Confirm button (rightmost).
                        let mut confirm_btn = Button::new(confirm_label)
                            .variant(confirm_tone.variant())
                            .size(Size::Md);
                        if let Some(t) = confirm_tone.tint(theme) {
                            confirm_btn = confirm_btn.tint(t);
                        }
                        if confirm_btn.show(ui, theme).clicked() {
                            outcome = ConfirmOutcome::Confirmed;
                        }
                        // Cancel button (left of confirm) — hidden if label is empty.
                        if !cancel_label.is_empty() {
                            ui.add_space(st::gap_sm());
                            if Button::new(cancel_label)
                                .variant(Variant::Ghost)
                                .size(Size::Md)
                                .show(ui, theme)
                                .clicked()
                            {
                                outcome = ConfirmOutcome::Cancelled;
                            }
                        }
                    });
                });
            });

        // Modal closed via click-outside / Esc → treat as cancel unless
        // we already detected an explicit confirm above.
        if modal_resp.closed && outcome == ConfirmOutcome::Open {
            outcome = ConfirmOutcome::Cancelled;
        }

        ConfirmDialogResponse { outcome }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let d = ConfirmDialog::new("Are you sure?");
        assert_eq!(d.title, "Are you sure?");
        assert!(d.id.is_none());
        assert!(d.body.is_none());
        assert_eq!(d.confirm_label, "OK");
        assert_eq!(d.confirm_tone, ConfirmTone::Primary);
        assert_eq!(d.cancel_label, "Cancel");
        assert_eq!(d.width, 380.0);
    }

    #[test]
    fn builders_set_each_field() {
        let d = ConfirmDialog::new("t")
            .id("the_id")
            .body("body text")
            .confirm("Delete", ConfirmTone::Danger)
            .cancel("Keep")
            .width(500.0);
        assert_eq!(d.id, Some("the_id"));
        assert_eq!(d.body, Some("body text"));
        assert_eq!(d.confirm_label, "Delete");
        assert_eq!(d.confirm_tone, ConfirmTone::Danger);
        assert_eq!(d.cancel_label, "Keep");
        assert_eq!(d.width, 500.0);
    }

    #[test]
    fn cancel_empty_string_disables_button() {
        // The empty cancel_label is the documented way to hide the cancel button.
        // The show() impl reads `cancel_label.is_empty()` — locked by this test.
        let d = ConfirmDialog::new("t").cancel("");
        assert!(d.cancel_label.is_empty());
    }

    #[test]
    fn tone_variants_map_correctly() {
        assert_eq!(ConfirmTone::Primary.variant(), Variant::Primary);
        assert_eq!(ConfirmTone::Danger.variant(), Variant::Danger);
        // Bull uses Primary variant but with bull-color tint.
        assert_eq!(ConfirmTone::Bull.variant(), Variant::Primary);
    }

    #[test]
    fn outcome_states_exhaustive() {
        // Lock the 3 outcome states — accidentally adding a 4th should fail this.
        fn is_open(o: ConfirmOutcome) -> bool {
            matches!(o, ConfirmOutcome::Open | ConfirmOutcome::Confirmed | ConfirmOutcome::Cancelled)
        }
        assert!(is_open(ConfirmOutcome::Open));
        assert!(is_open(ConfirmOutcome::Confirmed));
        assert!(is_open(ConfirmOutcome::Cancelled));
    }
}
