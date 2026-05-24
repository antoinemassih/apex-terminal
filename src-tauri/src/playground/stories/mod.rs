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
pub mod disclosure;
pub mod overlays;
pub mod sliders;
pub mod forms;
pub mod data;
pub mod specialty;
pub mod panegrid;

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

/// Persistent state for the Disclosure story (tabs, pagination, tree, panels).
pub struct DisclosureState {
    pub tab_line:    usize,
    pub tab_seg:     usize,
    pub tab_filled:  usize,
    pub page_a:      usize,
    pub page_b:      usize,
    pub tree_state:  _scaffold_lib::ui_kit::widgets::TreeState,
    pub panel_a_open: bool,
    pub panel_b_open: bool,
}

impl Default for DisclosureState {
    fn default() -> Self {
        Self {
            tab_line:    0,
            tab_seg:     0,
            tab_filled:  0,
            page_a:      0,
            page_b:      2,
            tree_state:  Default::default(),
            panel_a_open: true,
            panel_b_open: false,
        }
    }
}

/// Persistent state for the Overlays story.
#[derive(Default)]
pub struct OverlaysState {
    pub modal_open:        bool,
    pub sheet_open:        bool,
    pub popover_open:      bool,
    pub context_menu_open: bool,
}

/// Persistent state for the Sliders story.
pub struct SlidersState {
    pub slider_a:    f32,
    pub slider_b:    f32,
    pub slider_c:    f32,
    pub price_range: (f32, f32),
    pub time_range:  (f32, f32),
    pub opacity_a:   f32,
    pub opacity_b:   f32,
}

impl Default for SlidersState {
    fn default() -> Self {
        Self {
            slider_a:    25.0,
            slider_b:    60.0,
            slider_c:    90.0,
            price_range: (10.0, 90.0),
            time_range:  (570.0, 960.0), // 09:30 – 16:00 in minutes
            opacity_a:   0.70,
            opacity_b:   0.50,
        }
    }
}

/// Persistent state for the Forms story.
pub struct FormsState {
    pub name_buf:       String,
    pub email_buf:      String,
    pub notes_buf:      String,
    pub notif_on:       bool,
    pub risk_pct_buf:   String,
    pub max_loss_buf:   String,
    pub order_type_idx: usize,
    pub slippage_val:   f64,
    pub pdt_mode:       bool,
    pub advanced_open:  bool,
    pub route_idx:      usize,
    pub premarket_on:   bool,
}

impl Default for FormsState {
    fn default() -> Self {
        Self {
            name_buf:       String::new(),
            email_buf:      String::new(),
            notes_buf:      String::new(),
            notif_on:       true,
            risk_pct_buf:   "1.0".into(),
            max_loss_buf:   "500".into(),
            order_type_idx: 1,
            slippage_val:   5.0,
            pdt_mode:       true,
            advanced_open:  false,
            route_idx:      0,
            premarket_on:   false,
        }
    }
}

/// Persistent state for the Data story.
pub struct DataState {
    pub table_state: _scaffold_lib::ui_kit::widgets::TableState,
    pub cal_date:    Option<chrono::NaiveDate>,
    pub date_val:    Option<chrono::NaiveDate>,
    pub time_val:    Option<chrono::NaiveTime>,
}

impl Default for DataState {
    fn default() -> Self {
        Self {
            table_state: Default::default(),
            cal_date:    None,
            date_val:    None,
            time_val:    None,
        }
    }
}

/// Persistent state for the Specialty story.
pub struct SpecialtyState {
    pub color_a:        egui::Color32,
    pub color_b:        egui::Color32,
    pub selected_theme: usize,
    pub sidebar_active: usize,
}

impl Default for SpecialtyState {
    fn default() -> Self {
        Self {
            color_a:        egui::Color32::from_rgb(88, 160, 255),
            color_b:        egui::Color32::from_rgba_unmultiplied(200, 80, 30, 180),
            selected_theme: 0,
            sidebar_active: 0,
        }
    }
}

/// Top-level container for all story state.
#[derive(Default)]
pub struct PlaygroundState {
    pub selection:  SelectionState,
    pub inputs:     InputsState,
    pub disclosure: DisclosureState,
    pub overlays:   OverlaysState,
    pub sliders:    SlidersState,
    pub forms:      FormsState,
    pub data:       DataState,
    pub specialty:  SpecialtyState,
    pub panegrid:   panegrid::PaneGridState,
}
