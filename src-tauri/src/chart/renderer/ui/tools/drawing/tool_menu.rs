//! Drawing tool submenu and template submenu — rendered inside `resp.context_menu`.
//!
//! Call [`show_drawing_tool_menu`] and [`show_template_menu`] from within
//! the `resp.context_menu(|ui| { ... })` closure in gpu.rs.

use crate::chart_renderer::gpu::Chart;
use crate::chart_renderer::gpu::Watchlist;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::MenuItem;
use crate::chart_renderer::theme_impl::active_theme;

/// Output from the drawing-tool submenu.
pub struct DrawingToolMenuOutput {
    /// If Some, set `chart.draw_tool` to this value and clear pending points.
    pub new_tool: Option<String>,
}

/// Show the DRAWING TOOLS section of the right-click context menu.
///
/// Call this from inside `resp.context_menu(|ui| { ... })`.
/// The caller is responsible for the surrounding `ui.label("DRAWING TOOLS")`.
pub fn show_drawing_tool_menu(
    ui: &mut egui::Ui,
    chart: &Chart,
    watchlist: &Watchlist,
) -> DrawingToolMenuOutput {
    let mut new_tool: Option<&'static str> = None;

    let ctx_hotkeys: Vec<(String, String)> = watchlist.hotkeys.iter()
        .filter(|hk| hk.action.starts_with("tool_"))
        .map(|hk| (hk.action["tool_".len()..].to_string(), hk.key_name.clone()))
        .collect();
    let ctx_shortcut = |tool: &str| -> String {
        ctx_hotkeys.iter().find(|(t, _)| t == tool).map(|(_, k)| k.clone()).unwrap_or_default()
    };

    let t = active_theme(ui.ctx());

    macro_rules! ctx_tool_btn {
        ($ui:expr, $label:expr, $tool:expr) => {{
            let sc = ctx_shortcut($tool);
            let item = if sc.is_empty() {
                MenuItem::new($label)
            } else {
                MenuItem::new($label).shortcut(sc)
            };
            if item.show($ui, &t).clicked() {
                new_tool = Some($tool); $ui.close_menu();
            }
        }};
    }

    MenuItem::new("Lines").show_menu(ui, &t, |ui| {
        ctx_tool_btn!(ui, "Trendline", "trendline");
        ctx_tool_btn!(ui, "H-Line", "hline");
        ctx_tool_btn!(ui, "Vertical Line", "vline");
        ctx_tool_btn!(ui, "Ray", "ray");
    });
    MenuItem::new("Channels").show_menu(ui, &t, |ui| {
        ctx_tool_btn!(ui, "Channel", "channel");
        ctx_tool_btn!(ui, "Fib Channel", "fibchannel");
        ctx_tool_btn!(ui, "Pitchfork", "pitchfork");
    });
    MenuItem::new("Fibonacci").show_menu(ui, &t, |ui| {
        ctx_tool_btn!(ui, "Fib Retracement", "fibonacci");
        ctx_tool_btn!(ui, "Fib Extension", "fibext");
        ctx_tool_btn!(ui, "Fib Time Zones", "fibtimezone");
        ctx_tool_btn!(ui, "Fib Arcs", "fibarc");
        ctx_tool_btn!(ui, "Gann Fan", "gannfan");
        ctx_tool_btn!(ui, "Gann Box", "gannbox");
    });
    MenuItem::new("Ranges").show_menu(ui, &t, |ui| {
        ctx_tool_btn!(ui, "H-Zone", "hzone");
        if MenuItem::new("Price Range").show(ui, &t).clicked() { new_tool = Some("pricerange"); ui.close_menu(); }
        if MenuItem::new("Risk/Reward").show(ui, &t).clicked() { new_tool = Some("riskreward"); ui.close_menu(); }
    });
    MenuItem::new("Computed").show_menu(ui, &t, |ui| {
        if MenuItem::new("Regression Channel").show(ui, &t).clicked() { new_tool = Some("regression"); ui.close_menu(); }
        if MenuItem::new("Anchored VWAP").show(ui, &t).clicked()      { new_tool = Some("avwap");      ui.close_menu(); }
    });
    MenuItem::new("Patterns").show_menu(ui, &t, |ui| {
        if MenuItem::new("XABCD Harmonic").show(ui, &t).clicked()         { new_tool = Some("xabcd");                 ui.close_menu(); }
        if MenuItem::new("Elliott Impulse").show(ui, &t).clicked()        { new_tool = Some("elliott_impulse");       ui.close_menu(); }
        if MenuItem::new("Elliott ABC").show(ui, &t).clicked()            { new_tool = Some("elliott_corrective");    ui.close_menu(); }
        if MenuItem::new("Elliott WXY").show(ui, &t).clicked()            { new_tool = Some("elliott_wxy");           ui.close_menu(); }
        if MenuItem::new("Elliott WXYXZ").show(ui, &t).clicked()          { new_tool = Some("elliott_wxyxz");         ui.close_menu(); }
        if MenuItem::new("Elliott Sub-Impulse").show(ui, &t).clicked()    { new_tool = Some("elliott_sub_impulse");   ui.close_menu(); }
        if MenuItem::new("Elliott Sub-Corrective").show(ui, &t).clicked() { new_tool = Some("elliott_sub_corrective"); ui.close_menu(); }
    });
    MenuItem::new("Markers").show_menu(ui, &t, |ui| {
        if MenuItem::new("Bar Marker").show(ui, &t).clicked() { new_tool = Some("barmarker"); ui.close_menu(); }
    });
    MenuItem::new("Annotations").show_menu(ui, &t, |ui| {
        ctx_tool_btn!(ui, "Text Note", "textnote");
    });

    DrawingToolMenuOutput {
        new_tool: new_tool.map(|s| s.to_string()),
    }
}

/// Output from the template submenu.
pub struct TemplateMenuOutput {
    /// If Some(i), apply template at index `i` from `watchlist.pane_templates`.
    pub apply_tmpl: Option<usize>,
    /// If true, save current pane as a new template.
    pub save_as_template: bool,
}

/// Show the TEMPLATES section of the right-click context menu.
///
/// Returns deferred actions — the caller (gpu.rs) applies them after the menu
/// closure releases borrows.
pub fn show_template_menu(
    ui: &mut egui::Ui,
    chart: &Chart,
    watchlist: &Watchlist,
) -> TemplateMenuOutput {
    let mut apply_tmpl: Option<usize> = None;

    let t = active_theme(ui.ctx());

    if !watchlist.pane_templates.is_empty() {
        MenuItem::new("Apply Template")
            .icon(Icon::STAR)
            .show_menu(ui, &t, |ui| {
                for (i, (name, _)) in watchlist.pane_templates.iter().enumerate() {
                    if MenuItem::new(name.as_str()).show(ui, &t).clicked() {
                        apply_tmpl = Some(i);
                        ui.close_menu();
                    }
                }
            });
    }

    let save_as_template = MenuItem::new("Save as Template")
        .icon(Icon::STAR)
        .show(ui, &t)
        .clicked();
    if save_as_template { ui.close_menu(); }

    TemplateMenuOutput { apply_tmpl, save_as_template }
}
