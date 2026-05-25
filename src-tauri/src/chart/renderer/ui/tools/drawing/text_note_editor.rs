//! Inline text-note editor overlay (shown when a TextNote drawing is placed/activated).

use egui::Context;
use crate::ui_kit::widgets::Input;
use crate::chart_renderer::theme_impl::active_theme;

/// Everything the text-note editor needs (read) plus mutable text buffer.
pub struct TextNoteCtx<'a> {
    pub ctx: &'a Context,
    /// Pixel-X of the note anchor.
    pub x: f32,
    /// Pixel-Y of the note anchor.
    pub y: f32,
    /// pane index (for unique egui Id).
    pub pane_idx: usize,
    /// The buffer being edited.
    pub text_buf: &'a mut String,
    /// Font size stored in the drawing.
    pub font_size: f32,
}

/// Actions to apply after the editor returns.
pub struct TextNoteOutput {
    /// Commit: text is ready; gpu.rs should save/persist.
    pub commit: Option<String>,
    /// Discard: text was empty — remove the drawing.
    pub discard: bool,
}

pub fn show_text_note_editor(c: TextNoteCtx<'_>) -> TextNoteOutput {
    let mut commit: Option<String> = None;
    let mut discard = false;

    egui::Area::new(egui::Id::new(format!("text_edit_note_{}", c.pane_idx)))
        .fixed_pos(egui::pos2(c.x, c.y))
        .order(egui::Order::Foreground)
        .show(c.ctx, |ui| {
            let theme = active_theme(c.ctx);
            let r = Input::new(c.text_buf)
                .font_size(c.font_size)
                .proportional(true)
                // Use theme text color so light themes get dark text on light bg.
                .text_color(theme.text)
                .width(200.0)
                .show(ui, &theme);
            r.request_focus(c.ctx);
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if c.text_buf.is_empty() {
                    discard = true;
                } else {
                    commit = Some(c.text_buf.clone());
                }
            }
        });

    TextNoteOutput { commit, discard }
}
