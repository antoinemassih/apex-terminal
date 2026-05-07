//! Developer-only widget gallery. Shows every ui_kit::widgets widget
//! with sample variants/sizes/states for visual QA.
//!
//! Toggle via Ctrl+Shift+G or a settings button — same pattern as
//! perf_hud.
//!
//! Don't put real trading workflows in here. It's a flat showcase.

use egui::{Color32, Id};

use crate::chart_renderer::gpu::Theme;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::ui_kit::widgets::tokens::{Size as KitSize, Variant};
use crate::ui_kit::widgets::{
    paint_shadow, Alert, Badge, Breadcrumb, BreadcrumbItem, BreadcrumbSep, Button, Calendar,
    Checkbox, ColorPicker, Column, ContextMenu, DatePicker, HoverCard, Input, Kbd, Label, Link,
    Modal, Pagination, PolishedFontWeight, PolishedLabel, Popover, Progress, Radio, Resizable,
    Select, Separator, ShadowSpec, Sheet, SheetSide, SheetSize, Sidebar, SidebarItem, SidebarStyle,
    Skeleton, Slider, Spinner, Stepper, Switch, TabItem, TabTreatment, Table, TableState, Tabs,
    Tag, TagTone, Tooltip, Tree, TreeNode, TreeState,
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
    radio: u8,
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
    cal_date: Option<chrono::NaiveDate>,
    dp_single: Option<chrono::NaiveDate>,
    dp_range: Option<(chrono::NaiveDate, chrono::NaiveDate)>,
    modal_open: bool,
    sheet_open: bool,
    popover_open: bool,
    color_compact: Color32,
    color_inline: Color32,
    tree_state: TreeState,
    table_state: TableState,
    resizable_split: f32,
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
            radio: 1,
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
            cal_date: None,
            dp_single: None,
            dp_range: None,
            modal_open: false,
            sheet_open: false,
            popover_open: false,
            color_compact: Color32::from_rgb(220, 100, 80),
            color_inline: Color32::from_rgb(80, 160, 220),
            tree_state: TreeState::default(),
            table_state: TableState::default(),
            resizable_split: 0.4,
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

// ── Tree sample types ────────────────────────────────────────────────────

#[derive(Clone)]
struct DemoNode {
    id: u64,
    depth: usize,
    has_children: bool,
    label: String,
}

impl TreeNode for DemoNode {
    fn id(&self) -> u64 { self.id }
    fn depth(&self) -> usize { self.depth }
    fn has_children(&self) -> bool { self.has_children }
    fn label(&self) -> &str { &self.label }
}

fn demo_tree() -> Vec<DemoNode> {
    vec![
        DemoNode { id: 1, depth: 0, has_children: true, label: "Watchlists".into() },
        DemoNode { id: 2, depth: 1, has_children: true, label: "Tech".into() },
        DemoNode { id: 3, depth: 2, has_children: false, label: "AAPL".into() },
        DemoNode { id: 4, depth: 2, has_children: false, label: "MSFT".into() },
        DemoNode { id: 5, depth: 1, has_children: true, label: "Energy".into() },
        DemoNode { id: 6, depth: 2, has_children: false, label: "XOM".into() },
        DemoNode { id: 7, depth: 0, has_children: true, label: "Drawings".into() },
        DemoNode { id: 8, depth: 1, has_children: false, label: "Trendline".into() },
    ]
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
        ui.add_space(6.0);
        Label::subheading("Radio").show(ui, theme);
        ui.horizontal(|ui| {
            Radio::new(&mut s.radio, 0u8).label("Option A").show(ui, theme);
            Radio::new(&mut s.radio, 1u8).label("Option B").show(ui, theme);
            Radio::new(&mut s.radio, 2u8).label("Option C").show(ui, theme);
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

    // 8. Pagination + Breadcrumb + Link + Stepper
    section(ui, theme, "8. Pagination + Breadcrumb + Link + Stepper");
    with_state(ui, |ui, s| {
        Label::subheading("Pagination (total=100, page_size=10)").show(ui, theme);
        Pagination::new(&mut s.pagination_page, 100)
            .show_first_last(true)
            .show(ui, theme);
    });
    ui.add_space(6.0);
    Label::subheading("Breadcrumb").show(ui, theme);
    let crumbs = [
        BreadcrumbItem::new("Home").icon(Icon::CHART_LINE),
        BreadcrumbItem::new("Watchlists"),
        BreadcrumbItem::new("Tech"),
        BreadcrumbItem::new("AAPL"),
    ];
    Breadcrumb::with_items(&crumbs)
        .separator(BreadcrumbSep::Chevron)
        .show(ui, theme);
    ui.add_space(6.0);
    Label::subheading("Link").show(ui, theme);
    ui.horizontal(|ui| {
        Link::new("Plain link").show(ui, theme);
        Link::new("External link").external(true).show(ui, theme);
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

    // 10. Calendar + DatePicker
    section(ui, theme, "10. Calendar + DatePicker");
    with_state(ui, |ui, s| {
        Label::subheading("Calendar (single)").show(ui, theme);
        Calendar::new(&mut s.cal_date)
            .id_salt("gallery_cal")
            .show(ui, theme);
        ui.add_space(6.0);
        Label::subheading("DatePicker triggers (click to open)").show(ui, theme);
        ui.horizontal(|ui| {
            DatePicker::new(&mut s.dp_single)
                .placeholder("Pick a date")
                .id_salt("gallery_dp_single")
                .show(ui, theme);
            DatePicker::range(&mut s.dp_range)
                .placeholder("Pick a range")
                .id_salt("gallery_dp_range")
                .show(ui, theme);
        });
    });

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

    // 12. Sidebar / Resizable
    section(ui, theme, "12. Sidebar / Resizable");
    with_state(ui, |ui, s| {
        ui.horizontal(|ui| {
            // Embedded sidebar (rail).
            ui.allocate_ui(egui::vec2(80.0, 180.0), |ui| {
                let items = [
                    SidebarItem::new("Chart", Icon::CHART_LINE),
                    SidebarItem::new("Orders", Icon::CIRCLE),
                    SidebarItem::new("Tape", Icon::CIRCLE),
                ];
                let mut active = 0usize;
                Sidebar::new(&mut active, &items)
                    .style(SidebarStyle::Rail)
                    .show(ui, theme);
            });
            ui.add_space(8.0);
            // Embedded sidebar (panel).
            ui.allocate_ui(egui::vec2(180.0, 180.0), |ui| {
                let items = [
                    SidebarItem::new("Watchlists", Icon::CHART_LINE),
                    SidebarItem::new("Drawings", Icon::CIRCLE).badge(2),
                    SidebarItem::new("Settings", Icon::GEAR),
                ];
                let mut active = 0usize;
                Sidebar::new(&mut active, &items)
                    .style(SidebarStyle::Panel)
                    .show(ui, theme);
            });
            ui.add_space(8.0);
            // Resizable split.
            ui.allocate_ui(egui::vec2(360.0, 180.0), |ui| {
                Resizable::horizontal(&mut s.resizable_split).show(
                    ui,
                    theme,
                    |ui| {
                        PolishedLabel::new("Left")
                            .size(KitSize::Lg)
                            .weight(PolishedFontWeight::Semibold)
                            .show(ui, theme);
                        ui.label("Lorem ipsum dolor sit amet.");
                    },
                    |ui| {
                        PolishedLabel::new("Right")
                            .size(KitSize::Lg)
                            .weight(PolishedFontWeight::Semibold)
                            .show(ui, theme);
                        ui.label("Consectetur adipiscing elit.");
                    },
                );
            });
        });
    });

    // 13. Modal / Sheet / Popover triggers
    section(ui, theme, "13. Modal / Sheet / Popover");
    with_state(ui, |ui, s| {
        ui.horizontal(|ui| {
            if Button::new("Open Modal")
                .variant(Variant::Primary)
                .show(ui, theme)
                .clicked()
            {
                s.modal_open = true;
            }
            if Button::new("Open Sheet (right)")
                .variant(Variant::Secondary)
                .show(ui, theme)
                .clicked()
            {
                s.sheet_open = true;
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

        Sheet::new()
            .open(&mut s.sheet_open)
            .side(SheetSide::Right)
            .size(SheetSize::Fixed(360.0))
            .title("Gallery Sheet")
            .id("gallery_sheet")
            .show(ui, theme, |ui| {
                Label::new("Sample sheet content (right side).").show(ui, theme);
            });
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

    // 15. ColorPicker
    section(ui, theme, "15. ColorPicker");
    with_state(ui, |ui, s| {
        ui.horizontal(|ui| {
            Label::new("Compact").show(ui, theme);
            ColorPicker::new(&mut s.color_compact)
                .compact(true)
                .show(ui, theme);
            ui.add_space(16.0);
            Label::new("Inline").show(ui, theme);
            ColorPicker::new(&mut s.color_inline)
                .inline(true)
                .show(ui, theme);
        });
    });

    // 16. Tree
    section(ui, theme, "16. Tree");
    with_state(ui, |ui, s| {
        // Make sure top-level nodes default to expanded so the demo isn't empty.
        if s.tree_state.expanded.is_empty() {
            s.tree_state.expand(1);
            s.tree_state.expand(2);
            s.tree_state.expand(7);
        }
        let items = demo_tree();
        Tree::new(&mut s.tree_state, &items).show(ui, theme);
    });

    // 17. Table
    section(ui, theme, "17. Table");
    with_state(ui, |ui, s| {
        let cols = [
            Column::new("Symbol").sortable(true),
            Column::new("Last").sortable(true),
            Column::new("Chg %").sortable(true),
        ];
        let rows: Vec<[String; 3]> = vec![
            ["AAPL".into(), "189.45".into(), "+1.20%".into()],
            ["MSFT".into(), "412.30".into(), "+0.85%".into()],
            ["GOOG".into(), "143.20".into(), "-0.42%".into()],
            ["NVDA".into(), "920.10".into(), "+2.30%".into()],
            ["TSLA".into(), "210.55".into(), "-1.15%".into()],
        ];
        Table::new(&cols, &rows, &mut s.table_state)
            .resizable(true)
            .row_render(|ui, theme, row, col_idx, _cell_rect| {
                if col_idx == 2 {
                    let c = if row[2].starts_with('+') { theme.bull() } else { theme.bear() };
                    Label::new(&row[2]).color(c).show(ui, theme);
                } else {
                    Label::new(&row[col_idx]).show(ui, theme);
                }
            })
            .show(ui, theme);
    });

    // 18. Shadow showcase
    section(ui, theme, "18. Shadow showcase");
    let presets: [(&str, ShadowSpec); 4] = [
        ("sm", ShadowSpec::sm()),
        ("md", ShadowSpec::md()),
        ("lg", ShadowSpec::lg()),
        ("xl", ShadowSpec::xl()),
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
                egui::FontId::proportional(14.0),
                theme.text(),
            );
        }
    });

    ui.add_space(24.0);
}
