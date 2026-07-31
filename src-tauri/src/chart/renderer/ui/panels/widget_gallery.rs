//! Developer-only widget gallery. Shows every ui_kit::widgets widget
//! with sample variants/sizes/states for visual QA.
//!
//! Toggle via Ctrl+Shift+G or a settings button — same pattern as
//! perf_hud.
//!
//! Don't put real trading workflows in here. It's a flat showcase.

use egui::Id;
use crate::chart::renderer::ui::style as st;

use crate::chart_renderer::gpu::Theme;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::ui_kit::widgets::tokens::{Size as KitSize, Variant};
use crate::ui_kit::widgets::{
    paint_shadow, Alert, Badge, Button, Checkbox, ContextMenu, HoverCard, Input, Kbd, Label,
    Modal, Pagination, PolishedFontWeight, PolishedLabel, Popover, Progress,
    Select, Separator, ShadowSpec, Skeleton, Slider, Spinner, Stepper, Switch, TabItem,
    TabTreatment, Tabs, Tag, TagTone, Tooltip,
};

// ── Persistent sample state, all in egui memory ─────────────────────────

#[derive(Clone)]
struct GalleryState {
    sw_a: bool,
    sw_b: bool,
    sw_dis: bool,
    cb_a: bool,
    cb_b: bool,
    cb_tri: crate::ui_kit::widgets::CheckState,
    cb_dis: bool,
    in_a: String,
    in_b: String,
    in_c: String,
    in_d: String,
    in_e: String,
    sel_single: usize,
    sel_multi: Vec<usize>,
    sel_custom: usize,
    tabs1: usize,
    tabs2: usize,
    tabs3: usize,
    tab_items: Vec<TabItem>,
    slider_v: f32,
    pagination_page: usize,
    modal_open: bool,
    popover_open: bool,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            sw_a: true,
            sw_b: false,
            sw_dis: false,
            cb_a: false,
            cb_b: true,
            cb_tri: crate::ui_kit::widgets::CheckState::Indeterminate,
            cb_dis: true,
            in_a: String::new(),
            in_b: String::new(),
            in_c: "100".into(),
            in_d: "secret".into(),
            in_e: "bad@".into(),
            sel_single: 0,
            sel_multi: vec![0, 2],
            sel_custom: 0,
            tabs1: 0,
            tabs2: 1,
            tabs3: 0,
            tab_items: vec![
                TabItem::new("Chart"),
                TabItem::new("DOM"),
                TabItem::new("Tape").badge(3),
            ],
            slider_v: 50.0,
            pagination_page: 5,
            modal_open: false,
            popover_open: false,
        }
    }
}

fn state_id() -> Id {
    Id::new("apex_widget_gallery_state")
}

fn with_state<R>(ui: &mut egui::Ui, f: impl FnOnce(&mut egui::Ui, &mut GalleryState) -> R) -> R {
    let id = state_id();
    let mut s: GalleryState = ui
        .ctx()
        .memory(|m| m.data.get_temp::<GalleryState>(id))
        .unwrap_or_default();
    let r = f(ui, &mut s);
    ui.ctx().memory_mut(|m| m.data.insert_temp(id, s));
    r
}

// ── Section helper ───────────────────────────────────────────────────────

fn section(ui: &mut egui::Ui, theme: &Theme, title: &str) {
    ui.add_space(12.0);
    Separator::horizontal()
        .with_label(title.to_string())
        .show(ui, theme);
    ui.add_space(6.0);
}

// ── Public entry ─────────────────────────────────────────────────────────

pub fn show_widget_gallery(ui: &mut egui::Ui, theme: &Theme) {
    PolishedLabel::new("Apex Widget Gallery")
        .size(KitSize::Lg)
        .weight(PolishedFontWeight::Semibold)
        .show(ui, theme);
    ui.label("Developer-only — every ui_kit::widgets widget with sample variants for visual QA.");

    // ★ Data Viz — the new stylable chart-primitive library.
    section(ui, theme, "\u{2605} Data Viz (chart primitives)");
    ui.label("Stylable, theme-aware charts — re-tint with ColorScheme, re-shape with StyleSystem (stroke/dash/fonts/proportions).");
    ui.add_space(8.0);
    viz_gallery(ui, theme);

    // 0. Subpixel AA A/B
    section(ui, theme, "0. Subpixel AA A/B");
    ui.label("Same text, two render paths. Subpixel runs through the custom wgpu pipeline; grayscale through egui's bilinear sampler.");
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            Label::subheading("Grayscale (default egui path)").show(ui, theme);
            for size in [KitSize::Xs, KitSize::Sm, KitSize::Md, KitSize::Lg] {
                PolishedLabel::new("The quick brown fox jumps over the lazy dog 0123456789 => != >=")
                    .size(size)
                    .weight(PolishedFontWeight::Medium)
                    .show(ui, theme);
            }
        });
        ui.add_space(24.0);
        ui.vertical(|ui| {
            Label::subheading("Subpixel AA (custom wgpu pipeline)").show(ui, theme);
            for size in [KitSize::Xs, KitSize::Sm, KitSize::Md, KitSize::Lg] {
                PolishedLabel::new("The quick brown fox jumps over the lazy dog 0123456789 => != >=")
                    .size(size)
                    .weight(PolishedFontWeight::Medium)
                    .subpixel(true)
                    .show(ui, theme);
            }
        });
    });

    // 1. Buttons
    section(ui, theme, "1. Buttons");
    crate::ui_kit::widgets::show_button_gallery(ui, theme);

    // 2. Form atoms
    section(ui, theme, "2. Form atoms");
    with_state(ui, |ui, s| {
        Label::subheading("Switch").show(ui, theme);
        ui.horizontal(|ui| {
            Switch::new(&mut s.sw_a).size(KitSize::Sm).label("Sm on").show(ui, theme);
            Switch::new(&mut s.sw_b).size(KitSize::Sm).label("Sm off").show(ui, theme);
            Switch::new(&mut s.sw_a.clone()).size(KitSize::Md).label("Md on").show(ui, theme);
            let mut off = false;
            Switch::new(&mut off).size(KitSize::Md).label("Md off").show(ui, theme);
            Switch::new(&mut s.sw_dis).label("Disabled").disabled(true).show(ui, theme);
        });
        ui.add_space(6.0);
        Label::subheading("Checkbox").show(ui, theme);
        ui.horizontal(|ui| {
            Checkbox::new(&mut s.cb_a).label("Off").show(ui, theme);
            Checkbox::new(&mut s.cb_b).label("On").show(ui, theme);
            Checkbox::tri(&mut s.cb_tri).label("Indeterminate").show(ui, theme);
            Checkbox::new(&mut s.cb_dis).label("Disabled").disabled(true).show(ui, theme);
        });
    });

    // 3. Inputs
    section(ui, theme, "3. Inputs");
    with_state(ui, |ui, s| {
        ui.horizontal_wrapped(|ui| {
            Input::new(&mut s.in_a).min_width(160.0).show(ui, theme);
            Input::new(&mut s.in_b)
                .placeholder("Search…")
                .leading_icon(Icon::MAGNIFYING_GLASS)
                .min_width(180.0)
                .show(ui, theme);
            Input::new(&mut s.in_c)
                .prefix("$")
                .suffix("USD")
                .min_width(140.0)
                .show(ui, theme);
            Input::new(&mut s.in_d)
                .password(true)
                .placeholder("password")
                .min_width(140.0)
                .show(ui, theme);
            Input::new(&mut s.in_e)
                .invalid(true)
                .helper_text("Invalid email address")
                .min_width(180.0)
                .show(ui, theme);
        });
    });

    // 4. Selects
    section(ui, theme, "4. Selects");
    with_state(ui, |ui, s| {
        let opts = ["DAY", "GTC", "IOC"];
        ui.horizontal_wrapped(|ui| {
            ui.vertical(|ui| {
                Label::new("Single").show(ui, theme);
                Select::new(&mut s.sel_single, &opts).min_width(120.0).show(ui, theme);
            });
            ui.vertical(|ui| {
                Label::new("Multi searchable").show(ui, theme);
                Select::multi(&mut s.sel_multi, &opts)
                    .searchable(true)
                    .min_width(160.0)
                    .show(ui, theme);
            });
            ui.vertical(|ui| {
                Label::new("Custom item_render").show(ui, theme);
                Select::new(&mut s.sel_custom, &opts)
                    .min_width(160.0)
                    .item_render(|ui, theme, label, _selected| {
                        Tag::new(*label).tone(TagTone::Accent).show(ui, theme);
                    })
                    .show(ui, theme);
            });
        });
    });

    // 5. Tags / Badges / Kbd
    section(ui, theme, "5. Tags / Badges / Kbd");
    Label::subheading("Tags — filled").show(ui, theme);
    ui.horizontal_wrapped(|ui| {
        for (name, tone) in [
            ("Neutral", TagTone::Neutral),
            ("Accent", TagTone::Accent),
            ("Bull", TagTone::Bull),
            ("Bear", TagTone::Bear),
            ("Warn", TagTone::Warn),
        ] {
            Tag::new(name).tone(tone).show(ui, theme);
        }
    });
    Label::subheading("Tags — outline").show(ui, theme);
    ui.horizontal_wrapped(|ui| {
        for (name, tone) in [
            ("Neutral", TagTone::Neutral),
            ("Accent", TagTone::Accent),
            ("Bull", TagTone::Bull),
            ("Bear", TagTone::Bear),
            ("Warn", TagTone::Warn),
        ] {
            Tag::new(name).tone(tone).outline(true).show(ui, theme);
        }
    });
    Label::subheading("Badges").show(ui, theme);
    ui.horizontal_wrapped(|ui| {
        Badge::count(3).show(ui, theme);
        Badge::count(99).max(99).show(ui, theme);
        Badge::dot().show(ui, theme);
        Badge::text("NEW").tone(TagTone::Accent).show(ui, theme);
    });
    Label::subheading("Kbd").show(ui, theme);
    ui.horizontal_wrapped(|ui| {
        Kbd::new("Ctrl+K").show(ui, theme);
        Kbd::sequence(&["Cmd", "Shift", "P"]).show(ui, theme);
    });

    // 6. Tabs
    section(ui, theme, "6. Tabs");
    with_state(ui, |ui, s| {
        let labels = ["Overview", "Positions", "Orders", "History"];
        Label::subheading("Line").show(ui, theme);
        Tabs::new(&mut s.tabs1, &labels)
            .treatment(TabTreatment::Line)
            .id_salt("gallery_tabs_line")
            .show(ui, theme);
        ui.add_space(4.0);
        Label::subheading("Segmented (addable + closable + reorderable)").show(ui, theme);
        let resp = Tabs::with_items(&mut s.tabs2, &mut s.tab_items)
            .treatment(TabTreatment::Segmented)
            .closable(true)
            .addable(true)
            .reorderable(true)
            .id_salt("gallery_tabs_seg")
            .show(ui, theme);
        // Honor close requests so the gallery doesn't visually accumulate.
        for idx in resp.closed.iter().rev() {
            if *idx < s.tab_items.len() {
                s.tab_items.remove(*idx);
            }
        }
        if resp.add_clicked {
            s.tab_items.push(TabItem::new("New"));
        }
        ui.add_space(4.0);
        Label::subheading("Filled").show(ui, theme);
        Tabs::new(&mut s.tabs3, &labels)
            .treatment(TabTreatment::Filled)
            .id_salt("gallery_tabs_filled")
            .show(ui, theme);
    });

    // 7. Sliders + Progress + Spinner + Skeleton
    section(ui, theme, "7. Sliders + Progress + Spinner + Skeleton");
    with_state(ui, |ui, s| {
        Label::subheading("Slider").show(ui, theme);
        Slider::new(&mut s.slider_v, 0.0_f32..=100.0)
            .ticks(&[0.0, 25.0, 50.0, 75.0, 100.0])
            .show_value(true)
            .label("Sample")
            .show(ui, theme);
    });
    ui.add_space(6.0);
    Label::subheading("Progress — linear").show(ui, theme);
    ui.horizontal(|ui| {
        Progress::linear(0.5).show(ui, theme);
        ui.add_space(16.0);
        Progress::linear_indeterminate().show(ui, theme);
    });
    ui.add_space(6.0);
    Label::subheading("Progress — circular").show(ui, theme);
    ui.horizontal(|ui| {
        Progress::circular(0.5).size(KitSize::Md).show(ui, theme);
        ui.add_space(16.0);
        Progress::circular_indeterminate().size(KitSize::Md).show(ui, theme);
        ui.add_space(16.0);
        Spinner::new().size(KitSize::Md).show(ui, theme);
    });
    ui.add_space(6.0);
    Label::subheading("Skeleton").show(ui, theme);
    ui.horizontal(|ui| {
        Skeleton::rect(120.0, 24.0).show(ui, theme);
        Skeleton::text(160.0).show(ui, theme);
        Skeleton::circle(28.0).show(ui, theme);
    });
    Skeleton::lines(3, 240.0).show(ui, theme);

    // 8. Pagination + Stepper
    section(ui, theme, "8. Pagination + Stepper");
    with_state(ui, |ui, s| {
        Label::subheading("Pagination (total=100, page_size=10)").show(ui, theme);
        Pagination::new(&mut s.pagination_page, 100)
            .show_first_last(true)
            .show(ui, theme);
    });
    ui.add_space(6.0);
    Label::subheading("Stepper — horizontal").show(ui, theme);
    Stepper::new(&["Order", "Confirm", "Filled", "Closed"], 2).show(ui, theme);
    ui.add_space(6.0);
    Label::subheading("Stepper — vertical").show(ui, theme);
    Stepper::new(&["Order", "Confirm", "Filled", "Closed"], 1)
        .vertical(true)
        .show(ui, theme);

    // 9. Alert
    section(ui, theme, "9. Alert");
    Alert::info("Informational message body.")
        .title("Heads up")
        .closable(true)
        .show(ui, theme);
    ui.add_space(4.0);
    Alert::success("Order filled at $123.45.")
        .title("Filled")
        .closable(true)
        .show(ui, theme);
    ui.add_space(4.0);
    Alert::warn("Volatility breaker armed — position size auto-reduced.")
        .title("Warning")
        .closable(true)
        .show(ui, theme);
    ui.add_space(4.0);
    Alert::error("Connection lost. Reconnecting…")
        .title("Error")
        .closable(true)
        .show(ui, theme);

    // 11. Tooltip + HoverCard
    section(ui, theme, "11. Tooltip + HoverCard");
    ui.horizontal(|ui| {
        let tip = Button::new("Hover here for tooltip")
            .variant(Variant::Secondary)
            .show(ui, theme);
        Tooltip::new("This is a Tooltip — short hint text.").show(ui, &tip, theme);

        let card = Button::new("Hover here for hover card")
            .variant(Variant::Secondary)
            .show(ui, theme);
        HoverCard::new().show(ui, &card, theme, |ui| {
            PolishedLabel::new("AAPL")
                .size(KitSize::Lg)
                .weight(PolishedFontWeight::Semibold)
                .show(ui, theme);
            Label::new("Apple Inc — last $189.45 (+1.2%)").show(ui, theme);
        });
    });

    // 13. Modal / Popover triggers
    section(ui, theme, "13. Modal / Popover");
    with_state(ui, |ui, s| {
        ui.horizontal(|ui| {
            if Button::new("Open Modal")
                .variant(Variant::Primary)
                .show(ui, theme)
                .clicked()
            {
                s.modal_open = true;
            }
            let pop_btn = Button::new("Toggle Popover")
                .variant(Variant::Secondary)
                .show(ui, theme);
            if pop_btn.clicked() {
                s.popover_open = !s.popover_open;
            }
            Popover::new()
                .open(&mut s.popover_open)
                .anchor(pop_btn.rect)
                .id("gallery_popover")
                .show(ui, theme, |ui| {
                    PolishedLabel::new("Popover")
                        .size(KitSize::Lg)
                        .weight(PolishedFontWeight::Semibold)
                        .show(ui, theme);
                    ui.label("Anchored content. Click outside to close.");
                });
        });

        if s.modal_open {
            let resp = Modal::new("GALLERY MODAL")
                .id("gallery_modal")
                .ctx(ui.ctx())
                .theme(theme)
                .size(egui::vec2(360.0, 180.0))
                .show(|ui| {
                    Label::new("This is a sample Modal body.").show(ui, theme);
                    ui.add_space(12.0);
                    Button::new("Close")
                        .variant(Variant::Primary)
                        .show(ui, theme)
                        .clicked()
                });
            if resp.inner.unwrap_or(false) || resp.closed {
                s.modal_open = false;
            }
        }

    });

    // 14. ContextMenu
    section(ui, theme, "14. ContextMenu");
    let cm_btn = Button::new("Right-click me")
        .variant(Variant::Secondary)
        .show(ui, theme);
    if cm_btn.secondary_clicked() {
        let pos = ui
            .input(|i| i.pointer.interact_pos())
            .unwrap_or(cm_btn.rect.left_bottom());
        ui.ctx().memory_mut(|m| {
            m.data
                .insert_temp::<egui::Pos2>(Id::new("gallery_cm_pos"), pos);
            m.data
                .insert_temp::<bool>(Id::new("gallery_cm_open"), true);
        });
    }
    let cm_open = ui
        .ctx()
        .memory(|m| m.data.get_temp::<bool>(Id::new("gallery_cm_open")))
        .unwrap_or(false);
    if cm_open {
        let pos = ui
            .ctx()
            .memory(|m| m.data.get_temp::<egui::Pos2>(Id::new("gallery_cm_pos")))
            .unwrap_or(cm_btn.rect.left_bottom());
        use crate::ui_kit::widgets::context_menu::{MenuItem, MenuItemWithShortcut};
        let _ = ContextMenu::new(theme).pos(pos).id("gallery_cm").show(ui, |mb| {
            mb.add_section("Actions");
            mb.add(MenuItem::new("Cut"));
            mb.add(MenuItem::new("Copy"));
            mb.add(MenuItemWithShortcut::new("Paste", "Ctrl+V"));
            mb.add_divider();
            mb.add(MenuItem::new("Delete"));
        });
        // Close on next click anywhere outside.
        if ui.ctx().input(|i| i.pointer.any_click()) {
            ui.ctx()
                .memory_mut(|m| m.data.insert_temp::<bool>(Id::new("gallery_cm_open"), false));
        }
    }

    // 15. Shadow showcase
    section(ui, theme, "15. Shadow showcase");
    let presets: [(&str, ShadowSpec); 4] = [
        ("sm", ShadowSpec::sm_themed(theme)),
        ("md", ShadowSpec::md_themed(theme)),
        ("lg", ShadowSpec::lg_themed(theme)),
        ("xl", ShadowSpec::xl_themed(theme)),
    ];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 32.0;
        for (name, spec) in presets {
            let (rect, _resp) =
                ui.allocate_exact_size(egui::vec2(120.0, 80.0), egui::Sense::hover());
            paint_shadow(ui.painter(), rect, spec);
            ui.painter().rect_filled(rect, 8.0, theme.surface());
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                name,
                egui::FontId::proportional(st::font_md_plus()),
                theme.text(),
            );
        }
    });

    ui.add_space(24.0);
}

/// Dedicated, lightweight viz gallery (no GPU-heavy subpixel-AA section, so it's
/// stable to leave open) — the canonical preview for the chart primitives.
pub fn show_chart_gallery(ui: &mut egui::Ui, theme: &Theme) {
    PolishedLabel::new("Chart Viz Gallery")
        .size(KitSize::Lg)
        .weight(PolishedFontWeight::Semibold)
        .show(ui, theme);
    ui.label("Stylable, theme-aware chart primitives — re-tint with ColorScheme, re-shape with StyleSystem.");
    ui.add_space(10.0);
    viz_gallery(ui, theme);
}

/// Grid of the new viz chart primitives with sample data, drawn with the active
/// theme so the per-theme styling is visible at a glance.
fn viz_gallery(ui: &mut egui::Ui, theme: &Theme) {
    use crate::chart_renderer::ui::overlays::viz::charts;
    use crate::chart_renderer::ui::overlays::viz::style::{ChartStyle, LinePattern, FillMode};
    use crate::ui_kit::sx::Tone;

    let cst = ChartStyle::resolve(theme);
    let cols = 4usize;
    let cell_w = 200.0f32;
    let cell_h = 132.0f32;
    let pad_x = 16.0f32;
    let title_h = 20.0f32;
    let row_h = cell_h + title_h + 12.0;
    let demos = 12usize;
    let rows = demos.div_ceil(cols);

    let size = egui::vec2(cols as f32 * cell_w + (cols as f32 - 1.0) * pad_x, rows as f32 * row_h);
    let (area, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(area);

    // Sample data.
    let bars_d = [3.0f32, 5.0, 2.0, 8.0, 6.0, 4.0, 7.0, 5.0, 3.0, 6.0];
    let series: Vec<f32> = (0..32).map(|i| {
        let x = i as f32 * 0.4;
        50.0 + 18.0 * (x).sin() + 7.0 * (x * 2.3).cos() + i as f32 * 0.3
    }).collect();
    let hist: Vec<f32> = (0..24).map(|i| {
        let x = (i as f32 - 11.0) / 5.0;
        (-x * x).exp()
    }).collect();
    let heat: Vec<f32> = (0..24).map(|i| ((i * 7 % 11) as f32 / 10.0)).collect();

    let cell_origin = |i: usize| -> (f32, f32) {
        let (r, c) = (i / cols, i % cols);
        (area.left() + c as f32 * (cell_w + pad_x), area.top() + r as f32 * row_h)
    };
    let title = |i: usize, s: &str| {
        let (x, y) = cell_origin(i);
        p.text(egui::pos2(x, y), egui::Align2::LEFT_TOP, s,
            egui::FontId::monospace(st::font_xs()), st::tint(theme, Tone::Dim, st::alpha_line()));
    };
    let cell = |i: usize| -> egui::Rect {
        let (x, y) = cell_origin(i);
        let r = egui::Rect::from_min_size(egui::pos2(x, y + title_h), egui::vec2(cell_w, cell_h));
        p.rect_stroke(r, st::radius_sm() as u8,
            egui::Stroke::new(st::stroke_thin(), st::tint(theme, Tone::Border, st::alpha_subtle())),
            egui::StrokeKind::Inside);
        r.shrink(12.0)
    };
    let centered = |i: usize| -> (egui::Pos2, f32) {
        let inner = cell(i);
        (inner.center(), inner.width().min(inner.height()) * 0.46)
    };

    title(0, "stat");
    charts::stat(&p, cell(0), "RSI", "72", "14-period", Tone::Bull, &cst, theme);
    title(1, "bars");
    charts::bars(&p, cell(1), &bars_d, Tone::Accent, &cst, theme);
    title(2, "area / line");
    charts::area_line(&p, cell(2), &series, Tone::Bull, &cst, theme);
    title(3, "histogram");
    charts::histogram(&p, cell(3), &hist, Tone::Warn, &cst, theme);

    title(4, "donut");
    { let (c, r) = centered(4); charts::donut(&p, c, r, &[5.0, 3.0, 2.0, 1.0], &cst, theme); }
    title(5, "pie");
    { let (c, r) = centered(5); charts::pie(&p, c, r, &[4.0, 3.0, 2.0, 1.5], &cst, theme); }
    title(6, "radar");
    { let (c, r) = centered(6); charts::radar(&p, c, r, &[0.8, 0.6, 0.95, 0.45, 0.7, 0.55], Tone::Accent, &cst, theme); }
    title(7, "heatmap");
    charts::heatmap(&p, cell(7), 4, 6, &heat, Tone::Bull, &cst, theme);

    title(8, "multi-ring");
    { let (c, r) = centered(8); charts::multiring(&p, c, r, &[0.82, 0.55, 0.3], &cst, theme); }
    title(9, "line · dashed");
    { let mut d = cst; d.pattern = LinePattern::Dashed; d.fill = FillMode::Hollow;
      charts::area_line(&p, cell(9), &series, Tone::Bear, &d, theme); }
    title(10, "line · dotted");
    { let mut d = cst; d.pattern = LinePattern::Dotted; d.fill = FillMode::Hollow;
      charts::area_line(&p, cell(10), &series, Tone::Accent, &d, theme); }
    title(11, "stat · bear");
    charts::stat(&p, cell(11), "\u{0394} DAY", "-1.8%", "vs prior close", Tone::Bear, &cst, theme);

    ui.add_space(8.0);
}
