//! Story modules — one file per widget category.
//!
//! Each exposes `pub fn show(ui: &mut egui::Ui, theme: &dyn ComponentTheme)`.
//! Mutable story state lives in [`PlaygroundState`] so the story fns stay
//! stateless and the Playground struct owns all persistence.

pub mod buttons;
pub mod selection;
pub mod inputs;
pub mod feedback;
pub mod labels;
pub mod panels;

// ── Per-story mutable state ────────────────────────────────────────────────────

/// Persistent state for the Selection story (checkboxes, radios, switches, etc.).
#[derive(Default)]
pub struct SelectionState {
    pub check_a: bool,
    pub check_b: bool,
    pub check_c: bool,
    pub tri_state: _scaffold_lib::ui_kit::widgets::CheckState,
    pub radio_val: usize,
    pub switch_a: bool,
    pub switch_b: bool,
    pub toggle_val: usize,
    pub seg_val: usize,
}

/// Persistent state for the Inputs story.
pub struct InputsState {
    pub text_buf: String,
    pub search_buf: String,
    pub textarea_buf: String,
    pub num_val: f64,
}

impl Default for InputsState {
    fn default() -> Self {
        Self {
            text_buf: String::new(),
            search_buf: String::new(),
            textarea_buf: String::new(),
            num_val: 42.0,
        }
    }
}

/// Top-level container for all story state.
#[derive(Default)]
pub struct PlaygroundState {
    pub selection: SelectionState,
    pub inputs: InputsState,
}
