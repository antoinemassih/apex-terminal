//! First-launch welcome wizard — 4 steps using the ui_kit Stepper inside a Modal.
//!
//! Gated by `UiSettings.has_seen_welcome == false`. Once dismissed (Skip or
//! Finish), the flag flips to `true` and the wizard never appears again.
//! A "Show welcome again" entry in Settings → Appearance re-arms it.
//!
//! ## Step overview
//! 1. **Welcome** — headline, key shortcuts bullet list.
//! 2. **Connect Broker** — IB card vs "skip for now". Flips `connection_panel_open` if chosen.
//! 3. **Risk limits** — daily loss cap (USD) + max position size (% of account).
//! 4. **Start trading** — close wizard, flip `has_seen_welcome = true`.

use egui::{Context, FontId, RichText, Vec2};
use crate::ui_kit::sx::Tone;

use crate::chart::renderer::gpu::Theme;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::widgets::button::Button;
use crate::ui_kit::widgets::modal::{Modal, Anchor, HeaderStyle};
use crate::ui_kit::widgets::number_stepper::NumberStepper;
use crate::ui_kit::widgets::stepper::Stepper;
use crate::ui_kit::widgets::tokens::{Size, Variant};

const WIZARD_W: f32 = 580.0;
const WIZARD_H: f32 = 520.0;

/// First-launch welcome wizard state.
pub struct WelcomeWizard {
    /// Whether the wizard window should be rendered this frame.
    pub open: bool,
    /// Current step index (0..=3).
    pub step: u8,
    /// True if the user clicked "Skip for now" on the broker step.
    pub broker_skipped: bool,
    /// Daily loss cap (USD) — populated in step 3, written to TradingDefaults on Finish.
    pub daily_loss_cap: f32,
    /// Max position size as % of account — populated in step 3, written to TradingDefaults on Finish.
    pub max_position_pct: f32,
}

impl WelcomeWizard {
    /// Construct from persisted UiSettings (so a partially-completed wizard resumes).
    pub fn from_settings(has_seen: bool, resume_step: u8) -> Self {
        Self {
            open: !has_seen,
            step: resume_step,
            broker_skipped: false,
            daily_loss_cap: 500.0,
            max_position_pct: 5.0,
        }
    }

    /// Called once per frame from the render loop.
    ///
    /// Returns `true` while the wizard should remain open.
    /// Returns `false` when the user clicked Finish or Skip — the caller
    /// should then set `ui_settings.has_seen_welcome = true`.
    pub fn show(
        &mut self,
        ctx: &Context,
        theme: &Theme,
        conn_panel_open: &mut bool,
    ) -> bool {
        if !self.open {
            return false;
        }

        const STEP_LABELS: &[&str] = &["Welcome", "Broker", "Risk", "Done"];

        let screen = ctx.screen_rect();
        let pos = egui::pos2(
            screen.center().x - WIZARD_W * 0.5,
            (screen.center().y - WIZARD_H * 0.5).max(40.0),
        );

        let mut stay_open = true;

        let resp = Modal::new("Welcome to Apex")
            .id("welcome_wizard")
            .ctx(ctx)
            .theme(theme)
            .size(Vec2::new(WIZARD_W, WIZARD_H))
            .anchor(Anchor::Window { pos: Some(pos) })
            .header_style(HeaderStyle::None)
            .separator(false)
            .show(|ui| {
                // ── Stepper at the top ────────────────────────────────────────
                ui.add_space(st::gap_md());
                ui.horizontal(|ui| {
                    ui.add_space(st::gap_lg());
                    Stepper::new(STEP_LABELS, self.step as usize)
                        .show_labels(true)
                        .size(Size::Sm)
                        .show(ui, theme);
                    ui.add_space(st::gap_lg());
                });
                ui.add_space(st::gap_md());

                // ── Separator ─────────────────────────────────────────────────
                let sep_col = st::tint(theme, Tone::Border, st::alpha_muted());
                st::dialog_separator(ui, 0.0, sep_col);
                ui.add_space(st::gap_md());

                // ── Step body ─────────────────────────────────────────────────
                let mut advance = false;
                let mut go_back = false;
                let mut dismiss = false;

                egui::ScrollArea::vertical()
                    .max_height(WIZARD_H - 160.0)
                    .show(ui, |ui| {
                        ui.set_width(WIZARD_W - st::gap_lg() * 2.0);
                        egui::Frame::NONE
                            .inner_margin(egui::Margin::symmetric(
                                st::gap_lg() as i8,
                                st::gap_sm() as i8,
                            ))
                            .show(ui, |ui| match self.step {
                                0 => draw_step_welcome(ui, theme),
                                1 => draw_step_broker(ui, theme, conn_panel_open, &mut self.broker_skipped, &mut advance),
                                2 => draw_step_risk(ui, theme, &mut self.daily_loss_cap, &mut self.max_position_pct),
                                3 => draw_step_done(ui, theme),
                                _ => {}
                            });
                    });

                // ── Footer action bar ─────────────────────────────────────────
                let (skip_label, back_label, next_label) = match self.step {
                    0 => (Some("Skip"), None,           "Next →"),
                    1 => (Some("Skip"), Some("← Back"), "Next →"),
                    2 => (None,         Some("← Back"), "Next →"),
                    3 => (None,         None,           "Get Started"),
                    _ => (None,         None,           "Next →"),
                };

                ui.add_space(st::gap_sm());
                {
                    let sep_col = st::tint(theme, Tone::Border, st::alpha_muted());
                    let avail = ui.available_width();
                    let (rect, _) = ui.allocate_exact_size(
                        egui::Vec2::new(avail, st::stroke_thin()),
                        egui::Sense::hover(),
                    );
                    if ui.is_rect_visible(rect) {
                        ui.painter().hline(
                            rect.x_range(),
                            rect.center().y,
                            egui::Stroke::new(st::stroke_thin(), sep_col),
                        );
                    }
                }
                ui.add_space(st::gap_md());

                ui.horizontal(|ui| {
                    // Skip (left-anchored)
                    if let Some(lbl) = skip_label {
                        if Button::new(lbl)
                            .variant(Variant::Ghost)
                            .size(Size::Md)
                            .show(ui, theme)
                            .clicked()
                        {
                            dismiss = true;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = st::gap_sm();

                        // Primary action (Next / Get Started) — rightmost
                        let next_variant = if self.step == 3 { Variant::Primary } else { Variant::Primary };
                        if Button::new(next_label)
                            .variant(next_variant)
                            .size(Size::Md)
                            .show(ui, theme)
                            .clicked()
                        {
                            if self.step >= 3 {
                                dismiss = true;
                            } else {
                                advance = true;
                            }
                        }

                        // Back button (left of primary, rendered right-to-left so second)
                        if let Some(lbl) = back_label {
                            if Button::new(lbl)
                                .variant(Variant::Ghost)
                                .size(Size::Md)
                                .show(ui, theme)
                                .clicked()
                            {
                                go_back = true;
                            }
                        }
                    });
                });

                ui.add_space(st::gap_md());

                // Apply navigation (outside the closures to avoid borrow conflicts)
                if advance && !dismiss {
                    self.step = (self.step + 1).min(3);
                }
                if go_back {
                    self.step = self.step.saturating_sub(1);
                }
                if dismiss {
                    stay_open = false;
                }
            });

        if resp.closed {
            stay_open = false;
        }

        stay_open
    }
}

// ── Step renderers ────────────────────────────────────────────────────────────

fn draw_step_welcome(ui: &mut egui::Ui, theme: &Theme) {
    // Big monospace headline
    ui.add_space(st::gap_md());
    ui.horizontal_centered(|ui| {
        ui.label(
            RichText::new("APEX TERMINAL")
                .font(FontId::new(28.0, egui::FontFamily::Monospace))
                .color(theme.accent)
                .strong(),
        );
    });

    ui.add_space(st::gap_sm());
    ui.horizontal_centered(|ui| {
        ui.label(
            RichText::new("A pro-grade trading terminal built for speed")
                .font(FontId::proportional(st::font_md()))
                .color(theme.dim),
        );
    });

    ui.add_space(st::gap_lg());

    // Quick wins bullet list
    let bullets: &[&str] = &[
        "Cmd+K opens the command palette",
        "Cmd+, opens Settings",
        "Alt+S focuses the symbol input",
        "Drag pane dividers to resize",
    ];
    for bullet in bullets {
        ui.horizontal(|ui| {
            ui.add_space(st::gap_md());
            ui.label(
                RichText::new("•")
                    .font(FontId::proportional(st::font_md()))
                    .color(theme.accent),
            );
            ui.add_space(st::gap_xs());
            ui.label(
                RichText::new(*bullet)
                    .font(FontId::proportional(st::font_md()))
                    .color(theme.text),
            );
        });
        ui.add_space(st::gap_xs());
    }
    ui.add_space(st::gap_md());
}

fn draw_step_broker(
    ui: &mut egui::Ui,
    theme: &Theme,
    conn_panel_open: &mut bool,
    broker_skipped: &mut bool,
    advance: &mut bool,
) {
    ui.add_space(st::gap_md());
    ui.label(
        RichText::new("Connect a broker")
            .font(FontId::proportional(st::font_lg()))
            .color(theme.text)
            .strong(),
    );
    ui.add_space(st::gap_sm());
    ui.label(
        RichText::new("Connect your brokerage account to enable live trading.")
            .font(FontId::proportional(st::font_md()))
            .color(theme.dim),
    );
    ui.add_space(st::gap_lg());

    // Two side-by-side broker cards
    let card_w = (WIZARD_W - st::gap_lg() * 2.0 - st::gap_md()) * 0.5;
    ui.horizontal(|ui| {
        // Interactive Brokers card — was hand-rolled `egui::Frame::NONE.fill.stroke.corner_radius.inner_margin`
        // (audit-flagged welcome wizard broker card). Migrated to OutlinedBox
        // which carries all those token references behind one builder.
        ui.allocate_ui(Vec2::new(card_w, 120.0), |ui| {
            crate::ui_kit::widgets::OutlinedBox::new()
                .fill(theme.toolbar_bg)
                .border(st::tint(theme, Tone::Border, 120))
                .radius_sm()
                .padding(st::gap_md())
                .show(ui, theme, |ui| {
                ui.set_width(card_w - st::gap_md() * 2.0);
                ui.label(
                    RichText::new("Interactive Brokers")
                        .font(FontId::proportional(st::font_md()))
                        .color(theme.text)
                        .strong(),
                );
                ui.add_space(st::gap_xs());
                ui.label(
                    RichText::new("Connect via TWS or IB Gateway")
                        .font(FontId::proportional(st::font_sm()))
                        .color(theme.dim),
                );
                ui.add_space(st::gap_sm());
                if Button::new("Open connection panel")
                    .variant(Variant::Primary)
                    .size(Size::Sm)
                    .show(ui, theme)
                    .clicked()
                {
                    *conn_panel_open = true;
                    *broker_skipped = false;
                    *advance = true;
                }
            });
        });

        ui.add_space(st::gap_md());

        // "Skip for now" card — same OutlinedBox migration as IBKR card above,
        // with a softer border alpha (80 vs 120) to de-emphasise the skip path.
        ui.allocate_ui(Vec2::new(card_w, 120.0), |ui| {
            crate::ui_kit::widgets::OutlinedBox::new()
                .fill(theme.toolbar_bg)
                .border(st::tint(theme, Tone::Border, 80))
                .radius_sm()
                .padding(st::gap_md())
                .show(ui, theme, |ui| {
                ui.set_width(card_w - st::gap_md() * 2.0);
                ui.label(
                    RichText::new("Skip for now")
                        .font(FontId::proportional(st::font_md()))
                        .color(theme.text)
                        .strong(),
                );
                ui.add_space(st::gap_xs());
                ui.label(
                    RichText::new("Continue in paper-trading mode")
                        .font(FontId::proportional(st::font_sm()))
                        .color(theme.dim),
                );
                ui.add_space(st::gap_sm());
                if Button::new("I'll do this later")
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .show(ui, theme)
                    .clicked()
                {
                    *broker_skipped = true;
                    *advance = true;
                }
            });
        });
    });
    ui.add_space(st::gap_md());
}

fn draw_step_risk(
    ui: &mut egui::Ui,
    theme: &Theme,
    daily_loss_cap: &mut f32,
    max_position_pct: &mut f32,
) {
    ui.add_space(st::gap_md());
    ui.label(
        RichText::new("Set your risk limits")
            .font(FontId::proportional(st::font_lg()))
            .color(theme.text)
            .strong(),
    );
    ui.add_space(st::gap_sm());
    ui.label(
        RichText::new(
            "These limits help protect your account. You can change them later in Settings → Trading.",
        )
        .font(FontId::proportional(st::font_md()))
        .color(theme.dim),
    );
    ui.add_space(st::gap_lg());

    // Daily loss cap row
    ui.horizontal(|ui| {
        ui.allocate_ui(Vec2::new(220.0, 24.0), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("Daily loss cap (USD)")
                        .font(FontId::proportional(st::font_sm()))
                        .color(st::tint(theme, Tone::Text, 180)),
                );
            });
        });
        ui.add_space(st::gap_md());
        NumberStepper::new(daily_loss_cap)
            .range(0.0_f32..=10000.0)
            .step(50.0)
            .prefix("$")
            .decimals(0)
            .width(120.0)
            .show(ui, theme);
    });
    ui.add_space(st::gap_xs());
    ui.horizontal(|ui| {
        ui.add_space(220.0 + st::gap_md());
        ui.label(
            RichText::new("Apex will warn you when this amount is reached in a single day.")
                .font(FontId::proportional(st::font_sm()))
                .color(theme.dim),
        );
    });

    ui.add_space(st::gap_md());

    // Max position pct row
    ui.horizontal(|ui| {
        ui.allocate_ui(Vec2::new(220.0, 24.0), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("Max position size (% of account)")
                        .font(FontId::proportional(st::font_sm()))
                        .color(st::tint(theme, Tone::Text, 180)),
                );
            });
        });
        ui.add_space(st::gap_md());
        NumberStepper::new(max_position_pct)
            .range(0.0_f32..=100.0)
            .step(1.0)
            .suffix("%")
            .decimals(0)
            .width(120.0)
            .show(ui, theme);
    });
    ui.add_space(st::gap_xs());
    ui.horizontal(|ui| {
        ui.add_space(220.0 + st::gap_md());
        ui.label(
            RichText::new("Restricts a single position to this percentage of your account equity.")
                .font(FontId::proportional(st::font_sm()))
                .color(theme.dim),
        );
    });
    ui.add_space(st::gap_md());
}

fn draw_step_done(ui: &mut egui::Ui, theme: &Theme) {
    ui.add_space(st::gap_lg());
    ui.horizontal_centered(|ui| {
        ui.label(
            RichText::new("You're all set!")
                .font(FontId::new(24.0, egui::FontFamily::Monospace))
                .color(theme.bull)
                .strong(),
        );
    });
    ui.add_space(st::gap_md());
    ui.horizontal_centered(|ui| {
        ui.label(
            RichText::new(
                "Apex is pre-seeded with SPY, QQQ, AAPL, MSFT, NVDA, TSLA and 6 more symbols.",
            )
            .font(FontId::proportional(st::font_md()))
            .color(theme.text),
        );
    });
    ui.add_space(st::gap_sm());
    ui.horizontal_centered(|ui| {
        ui.label(
            RichText::new("Run /help anytime to see available commands.")
                .font(FontId::proportional(st::font_md()))
                .color(theme.dim),
        );
    });
    ui.add_space(st::gap_lg());
}
