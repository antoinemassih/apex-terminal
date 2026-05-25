//! Playground top-level — Storybook-like widget showcase for ui_kit.
//!
//! Implements `eframe::App`. Every frame it:
//!   1. Applies the active PortableTheme to egui visuals.
//!   2. Paints a top bar with theme-switcher buttons (using ui_kit Button).
//!   3. Renders a tab strip for the story categories.
//!   4. Dispatches to the active story module inside a scroll area.

pub mod designer;
pub mod stories;

use _scaffold_lib::ui_kit::widgets::{
    Button, Size, Variant,
    theme::{ComponentTheme, PortableTheme, set_ambient_theme},
};
use _scaffold_lib::ui_kit::tokens as st;
use egui::{Color32, ScrollArea};

// ── Available themes ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeChoice {
    Dark,
    Light,
    Midnight,  // Deep blue-black — same palette shape as Dark but cooler
    Bauhaus,   // Warm light theme
}

impl ThemeChoice {
    const ALL: &'static [Self] = &[
        ThemeChoice::Dark,
        ThemeChoice::Light,
        ThemeChoice::Midnight,
        ThemeChoice::Bauhaus,
    ];

    fn label(self) -> &'static str {
        match self {
            ThemeChoice::Dark => "Dark",
            ThemeChoice::Light => "Light",
            ThemeChoice::Midnight => "Midnight",
            ThemeChoice::Bauhaus => "Bauhaus",
        }
    }

    fn portable_theme(self) -> PortableTheme {
        match self {
            ThemeChoice::Dark => PortableTheme::dark(),
            ThemeChoice::Light => PortableTheme::light(),
            ThemeChoice::Midnight => midnight(),
            ThemeChoice::Bauhaus => bauhaus(),
        }
    }
}

// Midnight — deep cool blue-black palette, higher-contrast accent.
pub fn midnight() -> PortableTheme {
    PortableTheme {
        accent:           Color32::from_rgb( 88, 160, 255),
        text:             Color32::from_rgb(230, 234, 242),
        dim:              Color32::from_rgb(130, 138, 158),
        border:           Color32::from_rgb( 38,  44,  60),
        border_variant:   Color32::from_rgb( 56,  64,  84),
        warn:             Color32::from_rgb(240, 175,  60),
        bull:             Color32::from_rgb( 80, 200, 120),
        bear:             Color32::from_rgb(220,  80,  90),
        bg:               Color32::from_rgb( 10,  12,  20),
        surface:          Color32::from_rgb( 18,  22,  36),
        element_hover:    Color32::from_rgba_unmultiplied(255, 255, 255, 14),
        element_active:   Color32::from_rgba_unmultiplied(255, 255, 255, 28),
        element_selected: Color32::from_rgba_unmultiplied( 88, 160, 255, 44),
        element_disabled: Color32::from_rgba_unmultiplied(255, 255, 255,  8),
        ghost_hover:      Color32::from_rgba_unmultiplied(255, 255, 255, 10),
        ghost_active:     Color32::from_rgba_unmultiplied(255, 255, 255, 22),
        icon:             Color32::from_rgb(200, 210, 230),
        icon_muted:       Color32::from_rgb(130, 138, 158),
        icon_disabled:    Color32::from_rgb( 70,  78,  98),
        icon_accent:      Color32::from_rgb( 88, 160, 255),
        shadow_color:     Color32::BLACK,
    }
}

// Bauhaus — warm light palette, dark-brown text.
fn bauhaus() -> PortableTheme {
    PortableTheme {
        accent:           Color32::from_rgb(200,  80,  30),
        text:             Color32::from_rgb( 38,  28,  20),
        dim:              Color32::from_rgb(130, 110,  90),
        border:           Color32::from_rgb(210, 198, 186),
        border_variant:   Color32::from_rgb(195, 182, 168),
        warn:             Color32::from_rgb(185,  95,   0),
        bull:             Color32::from_rgb( 80, 200, 120),
        bear:             Color32::from_rgb(220,  80,  90),
        bg:               Color32::from_rgb(248, 244, 238),
        surface:          Color32::from_rgb(236, 230, 222),
        element_hover:    Color32::from_rgba_unmultiplied(  0,   0,   0, 14),
        element_active:   Color32::from_rgba_unmultiplied(  0,   0,   0, 28),
        element_selected: Color32::from_rgba_unmultiplied(200,  80,  30, 40),
        element_disabled: Color32::from_rgba_unmultiplied(  0,   0,   0,  8),
        ghost_hover:      Color32::from_rgba_unmultiplied(  0,   0,   0, 10),
        ghost_active:     Color32::from_rgba_unmultiplied(  0,   0,   0, 22),
        icon:             Color32::from_rgb( 60,  48,  38),
        icon_muted:       Color32::from_rgb(140, 118,  96),
        icon_disabled:    Color32::from_rgb(190, 178, 162),
        icon_accent:      Color32::from_rgb(200,  80,  30),
        shadow_color:     Color32::from_rgb(160, 140, 120),
    }
}

// ── Story categories ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Story {
    Buttons,
    Selection,
    Inputs,
    Feedback,
    Labels,
    Panels,
    Disclosure,
    Overlays,
    Sliders,
    Forms,
    Data,
    Specialty,
    PaneGrid,
}

impl Story {
    const ALL: &'static [Self] = &[
        Story::Buttons,
        Story::Selection,
        Story::Inputs,
        Story::Feedback,
        Story::Labels,
        Story::Panels,
        Story::Disclosure,
        Story::Overlays,
        Story::Sliders,
        Story::Forms,
        Story::Data,
        Story::Specialty,
        Story::PaneGrid,
    ];

    fn label(self) -> &'static str {
        match self {
            Story::Buttons    => "Buttons",
            Story::Selection  => "Selection",
            Story::Inputs     => "Inputs",
            Story::Feedback   => "Feedback",
            Story::Labels     => "Labels",
            Story::Panels     => "Panels",
            Story::Disclosure => "Disclosure",
            Story::Overlays   => "Overlays",
            Story::Sliders    => "Sliders",
            Story::Forms      => "Forms",
            Story::Data       => "Data",
            Story::Specialty  => "Specialty",
            Story::PaneGrid   => "PaneGrid",
        }
    }
}

// ── Playground app ─────────────────────────────────────────────────────────────

pub struct Playground {
    theme_choice: ThemeChoice,
    active_story: Story,
    /// Per-story mutable state stored here so stories are pure render fns.
    state: stories::PlaygroundState,
    /// Designer mode — when `designer.open == true`, the playground sources
    /// its PortableTheme from `designer.scheme` instead of the fixed
    /// ThemeChoice presets, and renders the editor in a right SidePanel.
    designer: designer::DesignerState,
}

impl Playground {
    pub fn new() -> Self {
        Self {
            theme_choice: ThemeChoice::Dark,
            active_story: Story::Buttons,
            state: stories::PlaygroundState::default(),
            designer: designer::DesignerState::default(),
        }
    }

    fn apply_egui_theme(&self, ctx: &egui::Context, theme: &PortableTheme) {
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(theme.text());
        visuals.panel_fill = theme.bg();
        visuals.window_fill = theme.bg();
        visuals.extreme_bg_color = theme.surface();
        visuals.faint_bg_color = theme.surface();
        visuals.code_bg_color = theme.surface();
        visuals.widgets.noninteractive.bg_fill = theme.surface();
        visuals.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0, theme.text());
        visuals.widgets.inactive.bg_fill = theme.surface();
        visuals.widgets.hovered.bg_fill = theme.surface_raised();
        visuals.widgets.active.bg_fill = theme.color_layer_up(2);
        ctx.set_visuals(visuals);
    }
}

impl Default for Playground {
    fn default() -> Self { Self::new() }
}

impl eframe::App for Playground {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Theme source ──────────────────────────────────────────────────
        // When the Designer panel is open, the playground renders with the
        // editable ColorScheme. Otherwise it uses the fixed ThemeChoice
        // presets (Dark/Light/Midnight/Bauhaus).
        let theme: PortableTheme = if self.designer.open {
            designer::color_scheme_to_portable(&self.designer.scheme)
        } else {
            self.theme_choice.portable_theme()
        };

        // Push theme into egui visuals + ambient stash so widgets using
        // active_theme() / Widget impls find it.
        self.apply_egui_theme(ctx, &theme);
        set_ambient_theme(ctx, theme.clone());

        // ── Top bar ───────────────────────────────────────────────────────────
        egui::TopBottomPanel::top("pg_topbar")
            .frame(egui::Frame::NONE.fill(theme.surface()).inner_margin(egui::Margin::symmetric(12, 6)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Apex Playground")
                            .color(theme.text())
                            .size(st::font_md()),
                    );
                    ui.add_space(16.0);

                    // Theme switcher — ui_kit Buttons, dogfooding the kit.
                    // Disabled while Designer mode is on so the user sees
                    // their custom palette, not the fixed presets.
                    if !self.designer.open {
                        for choice in ThemeChoice::ALL {
                            let active = *choice == self.theme_choice;
                            let resp = Button::new(choice.label())
                                .variant(Variant::Chip)
                                .size(Size::Xs)
                                .active(active)
                                .show(ui, &theme);
                            if resp.clicked() {
                                self.theme_choice = *choice;
                            }
                        }
                    }

                    // Right-aligned: Designer toggle.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if self.designer.open { "✕ Close Designer" } else { "✎ Theme Designer" };
                        if Button::new(label)
                            .variant(if self.designer.open { Variant::Danger } else { Variant::Primary })
                            .size(Size::Xs)
                            .show(ui, &theme)
                            .clicked()
                        {
                            self.designer.open = !self.designer.open;
                        }
                    });
                });
            });

        // ── Story tab strip ───────────────────────────────────────────────────
        egui::TopBottomPanel::top("pg_tabs")
            .frame(egui::Frame::NONE.fill(theme.bg()).inner_margin(egui::Margin::symmetric(12, 4)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for story in Story::ALL {
                        let active = *story == self.active_story;
                        let resp = Button::new(story.label())
                            .variant(Variant::Tab)
                            .size(Size::Sm)
                            .active(active)
                            .show(ui, &theme);
                        if resp.clicked() {
                            self.active_story = *story;
                        }
                    }
                });
            });

        // ── Right-side Designer panel (only when open) ────────────────────
        if self.designer.open {
            egui::SidePanel::right("pg_designer")
                .min_width(280.0)
                .max_width(420.0)
                .default_width(320.0)
                .frame(
                    egui::Frame::NONE
                        .fill(theme.surface())
                        .stroke(egui::Stroke::new(1.0, theme.border()))
                        .inner_margin(egui::Margin::symmetric(12, 10)),
                )
                .show(ctx, |ui| {
                    ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                        designer::render_designer_panel(ui, &mut self.designer, &theme);
                    });
                });
        }

        // ── Central scrollable body ───────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme.bg()).inner_margin(egui::Margin::same(16)))
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        match self.active_story {
                            Story::Buttons    => stories::buttons::show(ui, &theme),
                            Story::Selection  => stories::selection::show(ui, &theme, &mut self.state.selection),
                            Story::Inputs     => stories::inputs::show(ui, &theme, &mut self.state.inputs),
                            Story::Feedback   => stories::feedback::show(ui, &theme),
                            Story::Labels     => stories::labels::show(ui, &theme),
                            Story::Panels     => stories::panels::show(ui, &theme),
                            Story::Disclosure => stories::disclosure::show(ui, &theme, &mut self.state.disclosure),
                            Story::Overlays   => stories::overlays::show(ui, &theme, &mut self.state.overlays),
                            Story::Sliders    => stories::sliders::show(ui, &theme, &mut self.state.sliders),
                            Story::Forms      => stories::forms::show(ui, &theme, &mut self.state.forms),
                            Story::Data       => stories::data::show(ui, &theme, &mut self.state.data),
                            Story::Specialty  => stories::specialty::show(ui, &theme, &mut self.state.specialty),
                            Story::PaneGrid   => stories::panegrid::show(ui, &theme, &mut self.state.panegrid),
                        }
                    });
            });
    }
}
