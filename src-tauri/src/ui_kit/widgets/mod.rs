//! Reusable widgets for the chart UI.
//!
//! This module is the foundation for the new component library (port of
//! longbridge/gpui-component — see `THIRD_PARTY.md`). The submodules
//! `theme`, `tokens`, and `motion` define the contract widgets will be
//! built against in subsequent milestones. Existing free-function
//! helpers (color_picker, line_style_dropdown, etc.) are kept here for
//! now and will be migrated into kit widgets over time.

pub mod theme;
pub mod tokens;
pub mod motion;
pub mod button;
pub mod button_style;
pub mod modal;
pub mod toast;
pub mod context_menu;
pub mod placement;
pub mod tooltip;
pub mod popover;
pub mod hover_card;
pub mod switch;
pub mod checkbox;
pub mod radio;
pub mod input;
pub mod label;
pub mod polished_label;
pub mod text_engine;
pub mod text_subpixel_pipeline;
pub mod tag;
pub mod badge;
pub mod kbd;
pub mod separator;
pub mod sheet;
pub mod tabs;
pub mod slider;
pub mod progress;
pub mod spinner;
pub mod skeleton;
pub mod shadow;
pub mod shadow_pipeline;
pub mod select;
pub mod table;
pub mod pagination;
pub mod breadcrumb;
pub mod link;
pub mod alert;
pub mod stepper;
pub mod number_stepper;
pub mod metric_row;
pub mod tree;
pub mod sidebar;
pub mod resizable;
pub mod scroll_area;
pub mod calendar;
pub mod date_picker;
pub mod color_picker;
pub mod indicator;
pub mod toggle_row;
pub mod theme_preview_card;
pub mod selectable_row;
pub mod opacity_picker;
pub mod risk_reward_bar;
pub mod heatmap_grid;
pub mod trade_card;
pub mod guild_avatar_grid;
pub mod text_area;
pub mod search_input;
pub mod segmented_control;
pub mod toggle_group;
pub mod tag_input;
pub mod time_picker;
pub mod range_slider;
pub mod form_row;
pub mod form_section;
// P2: forms foundation primitives
pub mod form_field;
pub mod form_action_bar;
pub mod input_group;
pub mod fieldset;
pub mod panel;
pub mod header;
// Outer side-panel chrome (Agent J) — physically moved to
// `chart_renderer::ui::panels` because these shells pull chart-app
// composites (Watchlist, pane_tabs_header_h, kit::PanelHeader*) that
// aren't portable primitives. Re-exported below for back-compat.
// Body content primitives (Agent K)
pub mod panel_section;
pub mod panel_empty;
pub mod panel_loading;
pub mod panel_list_row;
pub mod panel_card;
pub mod panel_key_value_row;
// Foundation extension wave 2 (Agent V)
pub mod panel_sub_section;
// DS-IMPL-2: new panel primitives (wave 3)
pub mod table_header;
// pill_row deleted P6.2 — superseded by SegmentedControl + ToggleGroup; zero production callers.
pub mod outlined_box; // P6.4 — themed bordered container frame
pub mod confirm_dialog; // P6.4 — preset modal for "are you sure?" prompts
pub mod count_chip; // P6.6 — muted section-header count pill
pub mod status_pill;
pub mod panel_error;
pub mod panel_toolbar;
// P2: icon placement foundation (wire step)
pub mod icon_placement;
pub mod sparkline;
pub mod shell_variants;
pub mod menu_item;
pub mod toolbar_button;
pub mod frames;
pub mod pane_grid;

// ─── Foundation ──────────────────────────────────────────────────────
pub use tokens::{Size, Variant};
pub use shell_variants::{ButtonVariant, CardVariant, ChipVariant, RowVariant, InputVariant};
pub use shadow::{ShadowSpec, paint as paint_shadow, paint_gpu as paint_shadow_gpu};
pub use placement::{Align, Placement, Side};

// ─── Buttons & Links ─────────────────────────────────────────────────
pub use button::{Button, show_button_gallery};
pub use button_style::{ButtonStyle, ButtonState, DefaultButtonStyle};
pub use link::Link;
pub use menu_item::MenuItem;
pub use toolbar_button::ToolBarButton;

// ─── Inputs & Forms ──────────────────────────────────────────────────
pub use input::{Input, InputResponse};
pub use text_area::TextArea;
pub use search_input::SearchInput;
pub use select::{Select, SelectResponse};
pub use tag_input::TagInput;
pub use date_picker::{DatePicker, DatePickerResponse};
pub use time_picker::TimePicker;
pub use self::color_picker::ColorPicker;
pub use opacity_picker::{OpacityPicker, OPACITY_LEVELS as PICKER_OPACITY_LEVELS};
pub use number_stepper::NumberStepper;
pub use slider::Slider;
pub use range_slider::RangeSlider;
pub use form_row::{FormRow, FormRowAlign, FormRowCx};
pub use form_section::{FormSection, FieldSet, FormActions};
pub use form_field::{FormField, FormFieldResponse};
pub use form_action_bar::{FormActionBar, FormActionBarResponse, ActionBarPrimary};
pub use input_group::InputGroup;
pub use fieldset::Fieldset;

// ─── Toggles ─────────────────────────────────────────────────────────
pub use checkbox::{Checkbox, CheckState};
pub use radio::Radio;
pub use switch::Switch;
pub use toggle_row::ToggleRow;
pub use toggle_group::ToggleGroup;
pub use segmented_control::SegmentedControl;

// ─── Panel Ecosystem ─────────────────────────────────────────────────
pub use panel::{Panel, PanelCtx, PanelResponse};
pub use header::{Header, HeaderVariant, HeaderResponse};
// Chart-app panel composites (SidePanelShell, SplitSectionPanel) live in
// chart_renderer::ui::panels — the ui_kit re-export was removed in P4 to
// progress the workspace-crate extraction. Import from the canonical path:
//   use crate::chart_renderer::ui::panels::side_panel_shell::SidePanelShell;
//   use crate::chart_renderer::ui::panels::split_section_panel::SplitSectionPanel;
pub use panel_section::{
    PanelSection, PanelSectionGroup, PanelSectionGroupBuilder, SectionResponse, Tone as PanelTone,
};
pub use panel_sub_section::PanelSubSection;
pub use panel_card::PanelCard;
pub use panel_empty::PanelEmpty;
pub use panel_loading::PanelLoading;
pub use panel_error::PanelError;
pub use panel_list_row::{PanelListRow, PanelListRowResponse, TrailingBtn, TrailingTone};
pub use panel_list_row::{Column as PanelColumn, ColAlign as PanelColAlign};
pub use panel_key_value_row::PanelKeyValueRow;
pub use metric_row::{MetricRow, Tone as MetricTone};
pub use panel_toolbar::PanelToolbar;
pub use table_header::TableHeader;
pub use outlined_box::OutlinedBox;
pub use confirm_dialog::{ConfirmDialog, ConfirmDialogResponse, ConfirmOutcome, ConfirmTone};
pub use count_chip::{CountChip, CountChipTone};
pub use status_pill::StatusPill;

// ─── Surfaces ────────────────────────────────────────────────────────
pub use modal::Modal;
pub use sheet::{Sheet, SheetSide, SheetSize};
pub use popover::Popover;
pub use hover_card::HoverCard;
pub use tooltip::{Tooltip, PainterTooltip, paint_tooltip_card};
pub use context_menu::ContextMenu;
pub use alert::{Alert, AlertVariant, AlertResponse};
pub use toast::Toast;

// ─── Data / Display ──────────────────────────────────────────────────
pub use table::{ColAlign, ColWidth, Column, SortDir, Table, TableResponse, TableState};
pub use pagination::Pagination;
pub use tree::{Tree, TreeNode, TreeState, TreeResponse};
pub use tabs::{Tabs, TabsResponse, TabItem, TabTreatment, TabAlign};
pub use breadcrumb::{Breadcrumb, BreadcrumbItem, BreadcrumbSep, BreadcrumbResponse};
pub use progress::Progress;
pub use spinner::Spinner;
pub use skeleton::Skeleton;
pub use indicator::{Indicator, IndicatorStyle, IndicatorTone};
pub use badge::Badge;
pub use tag::{Tag, TagTone, TagResponse};
pub use kbd::Kbd;
pub use separator::Separator;
pub use label::Label;
pub use polished_label::{PolishedLabel, FontWeight as PolishedFontWeight};
pub use stepper::Stepper;
pub use calendar::{Calendar, CalendarResponse};

// ─── Icons ───────────────────────────────────────────────────────────
pub use icon_placement::{IconPlacement, IconTone, IconState, icon_glyph_color, icon_hover_bg};

// ─── Specialist / Domain ─────────────────────────────────────────────
pub use guild_avatar_grid::{GuildAvatarGrid, GuildEntry};
pub use heatmap_grid::{HeatmapGrid, HeatmapCell};
pub use trade_card::{TradeCard, TradeCardData};
pub use risk_reward_bar::RiskRewardBar;
pub use sparkline::{Sparkline, SparkStyle};
pub use sidebar::{Sidebar, SidebarStyle, SidebarItem, SidebarSection};
pub use resizable::Resizable;
pub use scroll_area::{ScrollDirection, ThemedScrollArea};
pub use theme_preview_card::ThemePreviewCard;
pub use selectable_row::SelectableRow;
pub use pane_grid::{PaneGrid, PaneState, PaneId, SplitId, Axis as PaneAxis};

use egui::{Ui, RichText};
use super::icons::Icon;
use crate::ui_kit::LineStyle;

/// Line style dropdown — shows visual preview, returns new style if changed.
pub fn line_style_dropdown(ui: &mut Ui, id: &str, current: LineStyle) -> Option<LineStyle> {
    let mut result = None;
    let label = match current { LineStyle::Solid => "____", LineStyle::Dashed => "- - -", LineStyle::Dotted => ". . ." };
    egui::ComboBox::from_id_salt(id).selected_text(label).width(65.0).show_ui(ui, |ui| {
        if ui.selectable_label(current == LineStyle::Solid, "_____ Solid").clicked() { result = Some(LineStyle::Solid); }
        if ui.selectable_label(current == LineStyle::Dashed, "- - - -  Dash").clicked() { result = Some(LineStyle::Dashed); }
        if ui.selectable_label(current == LineStyle::Dotted, ". . . . .  Dot").clicked() { result = Some(LineStyle::Dotted); }
    });
    result
}

/// Thickness dropdown — returns new thickness if changed.
pub fn thickness_dropdown(ui: &mut Ui, id: &str, current: f32) -> Option<f32> {
    let mut result = None;
    egui::ComboBox::from_id_salt(id).selected_text(format!("{:.1}px", current)).width(52.0).show_ui(ui, |ui| {
        for &th in &[0.5_f32, 1.0, 1.5, 2.5] {
            if ui.selectable_label((current - th).abs() < 0.1, format!("{:.1}px", th)).clicked() { result = Some(th); }
        }
    });
    result
}

/// Opacity dropdown — returns new opacity if changed.
pub fn opacity_dropdown(ui: &mut Ui, id: &str, current: f32) -> Option<f32> {
    let mut result = None;
    egui::ComboBox::from_id_salt(id).selected_text(format!("{}%", (current * 100.0) as u32)).width(48.0).show_ui(ui, |ui| {
        for &op in &[1.0_f32, 0.75, 0.5, 0.25] {
            if ui.selectable_label((current - op).abs() < 0.01, format!("{}%", (op * 100.0) as u32)).clicked() { result = Some(op); }
        }
    });
    result
}

/// Tool button — selectable button with icon for drawing tools.
pub fn tool_button(ui: &mut Ui, icon: &str, label: &str, active: bool) -> bool {
    let text = RichText::new(format!("{} {}", icon, label)).small();
    ui.selectable_label(active, text).clicked()
}

/// Delete button — red X icon.
pub fn delete_button(ui: &mut Ui) -> bool {
    button::Button::icon(Icon::X)
        .variant(tokens::Variant::Danger)
        .placement(IconPlacement::ListRow)
        .tone_destructive()
        .show(ui, &theme::active_theme(ui.ctx()))
        .on_hover_text("Delete")
        .clicked()
}

