//! Discord chat integration panel.
//!
//! Migrated to canonical ui_kit side-panel primitives:
//!   - Standalone `draw` uses `SidePanelShell` (was hand-rolled
//!     `SidePanel::left` + manual `PanelHeaderWithClose`).
//!   - Section headers ("Connect" splash / "Add Bot to Server" splash /
//!     channel-list categories / message stream) use `PanelSection` where
//!     they fit; the chat-bubble list keeps its bespoke
//!     `widgets::rows::ListRow` rendering because the per-author header +
//!     wrapping body don't map cleanly to `PanelListRow`.
//!   - Server avatar strip and category headers are unchanged.
//!
//! IMPORTANT: A handful of buttons keep `egui::Color32::WHITE` foregrounds —
//! those are intentional brand contrast over the Discord blurple fill (see
//! Agent C's notes). Do NOT theme-bind those — they're locked.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::components::text as widgets_text;
use crate::ui_kit::widgets::Input;
use crate::ui_kit::widgets::{Button, PanelEmpty, PanelListRow, PanelSection, Tooltip};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};
use crate::ui_kit::widgets::tokens::{Variant, Size};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::ui_kit::widgets::{GuildAvatarGrid, GuildEntry};
use super::super::super::gpu::{Watchlist, DiscordMessage, Theme};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::placement::Side;

/// Drain background Discord results (textures, messages, guilds). Call before rendering.
pub(crate) fn drain_background(ctx: &egui::Context, watchlist: &mut Watchlist) {
    // Check auth
    if !watchlist.discord.authenticated {
        if let Some(auth) = crate::discord::get_auth() {
            watchlist.discord.authenticated = true;
            watchlist.discord.username = auth.username.clone();
            watchlist.discord.user_id = auth.user_id.clone();
            watchlist.discord.connecting = false;
            crate::discord::fetch_guilds_bg();
        }
    }
    if let Some(guilds) = crate::discord::drain_guilds() {
        watchlist.discord.guilds = guilds;
    }
    for icon in crate::discord::drain_icons() {
        let pixels: Vec<egui::Color32> = icon.rgba.chunks_exact(4)
            .map(|c| egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
            .collect();
        let img = egui::ColorImage { size: [icon.width as usize, icon.height as usize], pixels };
        let tex = ctx.load_texture(format!("guild_{}", icon.guild_id), img, egui::TextureOptions::LINEAR);
        watchlist.discord.guild_icons.insert(icon.guild_id, tex);
    }
    if let Some(channels) = crate::discord::drain_channels() {
        watchlist.discord.channels = channels;
        watchlist.discord.channels_loading = false;
    }
    if let Some((msgs, is_append)) = crate::discord::drain_messages() {
        if is_append {
            for m in &msgs {
                let is_own = m.author.id == watchlist.discord.user_id;
                watchlist.discord.messages.push(DiscordMessage {
                    author: m.author.display_name().to_string(),
                    content: m.content.clone(),
                    timestamp: crate::discord::relative_time(&m.timestamp),
                    is_own,
                    has_chart: false,
                });
                watchlist.discord.last_msg_id = Some(m.id.clone());
            }
        } else {
            watchlist.discord.messages.clear();
            for m in &msgs {
                let is_own = m.author.id == watchlist.discord.user_id;
                watchlist.discord.messages.push(DiscordMessage {
                    author: m.author.display_name().to_string(),
                    content: m.content.clone(),
                    timestamp: crate::discord::relative_time(&m.timestamp),
                    is_own,
                    has_chart: false,
                });
            }
            watchlist.discord.last_msg_id = msgs.last().map(|m| m.id.clone());
            watchlist.discord.messages_loading = false;
        }
    }
    if let Some(result) = crate::discord::drain_send() {
        if let Ok(msg) = result {
            watchlist.discord.messages.push(DiscordMessage {
                author: msg.author.display_name().to_string(),
                content: msg.content.clone(),
                timestamp: "now".into(),
                is_own: msg.author.id == watchlist.discord.user_id,
                has_chart: false,
            });
            watchlist.discord.last_msg_id = Some(msg.id);
        }
    }
    if watchlist.discord.selected_channel.is_some()
        && !watchlist.discord.messages_loading
        && watchlist.discord.last_msg_id.is_some()
    {
        let should_poll = watchlist.discord.poll_timer
            .map(|t| t.elapsed().as_secs_f32() > 5.0)
            .unwrap_or(false);
        if should_poll {
            if let Some(ref ch_id) = watchlist.discord.selected_channel {
                crate::discord::fetch_messages_bg(ch_id.clone(), watchlist.discord.last_msg_id.clone());
                watchlist.discord.poll_timer = Some(std::time::Instant::now());
            }
        }
    }
}

/// Standalone left-rail Discord panel. Uses SidePanelShell with the channel
/// name as the dynamic title.
pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
    if !watchlist.discord.open { return; }
    drain_background(ctx, watchlist);

    // Title: channel name when a channel is open, otherwise "DISCORD".
    let title_owned = if !watchlist.discord.channel.is_empty() {
        watchlist.discord.channel.clone()
    } else {
        "DISCORD".to_string()
    };

    let resp = SidePanelShell::new("discord_chat", &title_owned)
        .width(Width::Narrow)
        .side(Side::Left)
        .show(ctx, t, |ui, t| {
            draw_body(ui, watchlist, t);
        });
    if resp.close_clicked { watchlist.discord.open = false; }
}

/// Tab body content (no SidePanel wrapper). Used by feed_panel Discord tab.
/// NOTE: caller must call `drain_background(ctx, watchlist)` before this each frame.
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    draw_body(ui, watchlist, t);
}

/// Shared body — auth splash, guild strip, channel list, message stream + input.
fn draw_body(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    let discord_blurple = super::super::style::discord_blurple();
    let panel_w = ui.available_width();

    if !watchlist.discord.authenticated {
        // ── Not authenticated: Connect splash ──
        let avail = ui.available_size();
        ui.allocate_ui_with_layout(
            egui::vec2(panel_w, avail.y),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.add_space(avail.y * 0.25);
                ui.label(egui::RichText::new(Icon::CHAT_DOTS).size(font_display_md()).color(color_half(discord_blurple)));
                ui.add_space(gap_md());
                if !crate::discord::is_configured() {
                    PanelEmpty::new("Discord not configured")
                        .hint("Add discord.env with credentials")
                        .show(ui, t);
                } else if watchlist.discord.connecting {
                    ui.add(widgets_text::MonospaceCode::new("Waiting for authorization...").xs().color(t.dim));
                    ui.add_space(gap_sm());
                    crate::ui_kit::widgets::Spinner::new().show(ui, t);
                    ui.add_space(gap_xs());
                    ui.add(widgets_text::MonospaceCode::new("Complete sign-in in your browser").xs().color(color_half(t.dim)));
                } else {
                    // WHITE foreground is intentional brand contrast on the Discord blurple fill.
                    if ui.add(Button::new("  Connect Discord  ")
                        .variant(Variant::Chrome)
                        .size(Size::Sm)
                        .fg(egui::Color32::WHITE)
                        .fill(discord_blurple)
                        .corner_radius(current().r_lg as f32)
                        // Off-ladder 36.0 → nearest rung Lg (34). This floor IS
                        // active (size Sm = 22), so it is a real -2px on a
                        // standalone empty-state CTA — traded for density-awareness.
                        .min_size(egui::vec2(180.0, Size::Lg.height()))
                    ).clicked() {
                        watchlist.discord.connecting = true;
                        crate::discord::start_oauth2();
                    }
                    ui.add_space(gap_sm());
                    ui.add(widgets_text::MonospaceCode::new("Chat with your trading community").xs().color(color_half(t.dim)));
                }
            },
        );
        return;
    }

    // ── Authenticated: full Discord UI ──
    let has_guilds = !watchlist.discord.guilds.is_empty();

    // Disconnect action (channel-name + close are provided by shell when
    // standalone; when embedded the parent feed_panel header carries the
    // close). Disconnect lives inline at the top.
    ui.horizontal(|ui| {
        ui.add_space(gap_xs());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(gap_xs());
            let r = Button::close()
                .tint(t.bear)
                .placement(IconPlacement::PanelHeader)
                .show(ui, t);
            Tooltip::new("Disconnect").show(ui, &r, t);
            if r.clicked() {
                crate::discord::disconnect();
                watchlist.discord.authenticated = false;
                watchlist.discord.username.clear();
                watchlist.discord.user_id.clear();
                watchlist.discord.guilds.clear();
                watchlist.discord.selected_guild = None;
                watchlist.discord.channels.clear();
                watchlist.discord.selected_channel = None;
                watchlist.discord.channel.clear();
                watchlist.discord.messages.clear();
                watchlist.discord.guild_icons.clear();
                watchlist.discord.last_msg_id = None;
                watchlist.discord.poll_timer = None;
            }
        });
    });

    // ── Server strip (horizontal, top) ──
    if has_guilds {
        let strip_h = 44.0;
        let (strip_bg_rect, _) = ui.allocate_exact_size(egui::vec2(panel_w, 0.0), egui::Sense::hover());
        let bg_rect = egui::Rect::from_min_size(strip_bg_rect.min, egui::vec2(panel_w, strip_h));
        // Was Color32::BLACK + alpha_tint — broke 4 light themes. Theme-aware
        // shadow color reads as "darker than chart bg" everywhere.
        ui.painter().rect_filled(bg_rect, 0.0, shadow_color_alpha(t, alpha_tint()));

        egui::ScrollArea::horizontal().id_salt("guild_strip").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(gap_sm());
                let guild_entries: Vec<GuildEntry<'_>> = watchlist.discord.guilds.iter()
                    .map(|g| GuildEntry { id: &g.id, name: &g.name })
                    .collect();
                let mut clicked_guild: Option<String> = None;
                GuildAvatarGrid::new(&guild_entries, &watchlist.discord.guild_icons)
                    .selected(watchlist.discord.selected_guild.as_deref())
                    .on_click(|id| { clicked_guild = Some(id.to_string()); })
                    .show(ui, t);
                if let Some(id) = clicked_guild {
                    watchlist.discord.selected_guild = Some(id.clone());
                    watchlist.discord.channels.clear();
                    watchlist.discord.selected_channel = None;
                    watchlist.discord.messages.clear();
                    watchlist.discord.last_msg_id = None;
                    watchlist.discord.channel.clear();
                    watchlist.discord.channels_loading = true;
                    crate::discord::fetch_channels_bg(id);
                }
                ui.add_space(gap_xs());
            });
        });
        ui.add_space(gap_xs());
        let sep_rect = ui.allocate_exact_size(egui::vec2(panel_w, 1.0), egui::Sense::hover()).0;
        ui.painter().rect_filled(sep_rect, 0.0, tint(t, Tone::Border, alpha_dim()));
    }

    // ── Content area ──
    let content_w = panel_w;

    if watchlist.discord.selected_guild.is_none() {
        let avail = ui.available_size();
        ui.allocate_ui_with_layout(
            egui::vec2(content_w, avail.y),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.add_space(avail.y * 0.3);
                PanelEmpty::new("Select a server")
                    .hint("from the icons above")
                    .show(ui, t);
            },
        );
    } else if watchlist.discord.selected_channel.is_none() {
        // Server selected, show channel list
        if watchlist.discord.channels_loading {
            ui.horizontal(|ui| {
                ui.add_space(gap_sm());
                crate::ui_kit::widgets::Spinner::new().show(ui, t);
                ui.add(widgets_text::MonospaceCode::new("Loading channels...").xs().color(t.dim));
            });
        } else if watchlist.discord.channels.is_empty() {
            // Bot not in this server — show invite button
            let avail = ui.available_size();
            ui.allocate_ui_with_layout(
                egui::vec2(content_w, avail.y),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.add_space(avail.y * 0.2);
                    ui.label(egui::RichText::new(Icon::PLUGS_CONNECTED).size(font_display_md()).color(color_half(t.dim)));
                    ui.add_space(gap_sm());
                    ui.add(widgets_text::MonospaceCode::new("Bot not in this server").xs().color(t.dim));
                    ui.add_space(gap_xs());
                    ui.add(widgets_text::MonospaceCode::new("Add the Apex bot to enable\nchannels & messaging").xs().color(color_half(t.dim)));
                    ui.add_space(gap_sm());
                    // WHITE foreground is intentional brand contrast on the Discord blurple fill.
                    if ui.add(Button::new("  Add Bot to Server  ")
                        .variant(Variant::Chrome)
                        .size(Size::Sm)
                        .fg(egui::Color32::WHITE)
                        .fill(discord_blurple)
                        .corner_radius(current().r_sm as f32)
                        .min_size(egui::vec2(160.0, row_height_tall()))
                    ).clicked() {
                        let guild_id = watchlist.discord.selected_guild.as_deref().unwrap_or("");
                        let url = format!(
                            "https://discord.com/oauth2/authorize?client_id=1492118514482417776&scope=bot&permissions=68608&guild_id={}",
                            guild_id
                        );
                        let _ = open::that(&url);
                    }
                    ui.add_space(gap_sm());
                    ui.add(widgets_text::CaptionLabel::new("Server admins can also\nadd the bot themselves").color(t.dim).gamma(0.4));
                    ui.add_space(gap_md());
                    if ui.add(Button::new("Retry")
                        .variant(Variant::Ghost)
                        .size(Size::Xs)
                    ).clicked() {
                        if let Some(ref gid) = watchlist.discord.selected_guild {
                            watchlist.discord.channels_loading = true;
                            crate::discord::fetch_channels_bg(gid.clone());
                        }
                    }
                },
            );
        } else {
            // Channel list — group by category, each category as a PanelSection.
            let channels: Vec<_> = watchlist.discord.channels.clone();
            egui::ScrollArea::vertical().id_salt("discord_channels").show(ui, |ui| {
                ui.set_min_width(content_w - 4.0);
                // Pre-bucket channels into (category_name, [text_channels]).
                let mut buckets: Vec<(String, Vec<&crate::discord::DiscordChannel>)> = Vec::new();
                for ch in &channels {
                    if ch.is_category() {
                        let name = ch.name.as_deref().unwrap_or("UNKNOWN").to_uppercase();
                        buckets.push((name, Vec::new()));
                    } else if ch.is_text() {
                        if buckets.is_empty() {
                            buckets.push(("CHANNELS".into(), Vec::new()));
                        }
                        buckets.last_mut().unwrap().1.push(ch);
                    }
                }
                let mut clicked_channel: Option<(String, String)> = None;
                for (cat_name, text_chs) in &buckets {
                    if text_chs.is_empty() { continue; }
                    PanelSection::new(cat_name).rule(false).show(ui, t, |ui, t| {
                        // M0.6: was t.dim.gamma_multiply(1.3) — direction-blind on light
                        // themes (brightening dim REDUCES contrast there). Faded text
                        // reads as "between dim and text" on any palette.
                        let ch_text = tint(t, Tone::Text, alpha_solid());
                        for ch in text_chs {
                            let name = ch.name.as_deref().unwrap_or("unknown").to_string();
                            let dim = t.dim;
                            let row_id = format!("discord_ch_{}", ch.id);
                            let name_for_leading = name.clone();
                            let resp = PanelListRow::new(&row_id)
                                .leading(move |ui, _t| {
                                    ui.add(widgets_text::MonospaceCode::new("#").xs().color(dim));
                                    ui.add(widgets_text::MonospaceCode::new(&name_for_leading).xs().color(ch_text));
                                })
                                .show(ui, t);
                            if resp.clicked() {
                                clicked_channel = Some((ch.id.clone(), name));
                            }
                        }
                    });
                }
                if let Some((id, name)) = clicked_channel {
                    watchlist.discord.selected_channel = Some(id.clone());
                    watchlist.discord.channel = format!("# {}", name);
                    watchlist.discord.messages.clear();
                    watchlist.discord.last_msg_id = None;
                    watchlist.discord.messages_loading = true;
                    watchlist.discord.poll_timer = None;
                    crate::discord::fetch_messages_bg(id, None);
                }
            });
        }
    } else {
        // ── Channel selected: show messages + input ──
        ui.add_space(gap_xs());

        // Back button + channel name
        ui.horizontal(|ui| {
            ui.add_space(gap_sm());
            let r = ui.add(Button::new(Icon::CARET_RIGHT)
                .variant(Variant::TextOnly)
                .size(Size::Sm)
                .fg(t.dim));
            Tooltip::new("Back to channels").show(ui, &r, t);
            if r.clicked() {
                watchlist.discord.selected_channel = None;
                watchlist.discord.messages.clear();
                watchlist.discord.last_msg_id = None;
                watchlist.discord.poll_timer = None;
            }
            ui.add(widgets_text::MonospaceCode::new(&watchlist.discord.channel).xs().color(t.dim));
        });
        ui.add_space(gap_xs());

        // Messages — chat-bubble list intentionally bespoke (per Agent I's audit).
        let input_h = 36.0;
        let msg_area_h = ui.available_height() - input_h;
        egui::ScrollArea::vertical()
            .id_salt("discord_msgs")
            .max_height(msg_area_h.max(60.0))
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.set_min_width(content_w - 4.0);
                if watchlist.discord.messages_loading {
                    ui.add_space(gap_sm());
                    let row_w = (content_w - 24.0).max(80.0);
                    for _ in 0..5 {
                        ui.add_space(gap_xs());
                        ui.horizontal(|ui| {
                            ui.add_space(gap_sm());
                            crate::ui_kit::widgets::Skeleton::circle(20.0).show(ui, t);
                            ui.add_space(gap_sm());
                            ui.vertical(|ui| {
                                crate::ui_kit::widgets::Skeleton::text(row_w * 0.35).show(ui, t);
                                ui.add_space(gap_2xs());
                                crate::ui_kit::widgets::Skeleton::text(row_w * 0.75).show(ui, t);
                            });
                        });
                        ui.add_space(gap_xs());
                    }
                } else if watchlist.discord.messages.is_empty() {
                    ui.add_space(gap_sm());
                    PanelEmpty::new("No messages in this channel").show(ui, t);
                }
                let mut prev_author = String::new();
                for msg in &watchlist.discord.messages {
                    if msg.content.is_empty() { continue; }
                    let author_hash = msg.author.bytes().fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
                    let author_col = chat_author_color(author_hash);
                    let same_author = msg.author == prev_author;

                    if !same_author {
                        if !prev_author.is_empty() { ui.add_space(gap_xs()); }
                        let author = msg.author.clone();
                        let timestamp = msg.timestamp.clone();
                        let dim = t.dim;
                        let row_id = format!("discord_msg_hdr_{}_{}", msg.author, msg.timestamp);
                        PanelListRow::new(&row_id)
                            .hoverable(false)
                            .height(18.0)
                            .leading(move |ui, _t| {
                                // Author color dot (matches legacy left_painter_circle).
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                                ui.painter().circle_filled(rect.center(), 3.0, author_col);
                                ui.add(widgets_text::MonospaceCode::new(&author).xs().strong(true).color(author_col));
                                ui.add(widgets_text::CaptionLabel::new(&timestamp).color(dim).gamma(0.4));
                            })
                            .show(ui, t);
                    }
                    // Message content — INLINE EXCEPTION: egui::Label required for
                    // wrap_mode(Wrap); no widget currently supports wrapping body text.
                    ui.horizontal(|ui| {
                        ui.add_space(gap_xl()); // align with text after avatar dot
                        ui.add(egui::Label::new(
                            egui::RichText::new(&msg.content).monospace().size(font_sm()).color(t.text)
                        ).wrap_mode(egui::TextWrapMode::Wrap));
                    });
                    prev_author = msg.author.clone();
                }
                ui.add_space(gap_xs());
            });

        // Input area
        let sep_rect = ui.allocate_exact_size(egui::vec2(content_w, 1.0), egui::Sense::hover()).0;
        ui.painter().rect_filled(sep_rect, 0.0, tint(t, Tone::Border, alpha_muted()));
        ui.add_space(gap_xs());
        ui.horizontal(|ui| {
            ui.add_space(gap_sm());
            let input = Input::new(&mut watchlist.discord.input)
                .width(content_w - 60.0)
                .text_color(t.text)
                .placeholder(format!("Message {}...", watchlist.discord.channel))
                .show(ui, t);
            // WHITE foreground is intentional brand contrast on the Discord blurple fill.
            let send_clicked = ui.add(Button::new("Send")
                .variant(Variant::Chrome)
                .size(Size::Sm)
                .fg(egui::Color32::WHITE)
                .fill(discord_blurple)
                .corner_radius(current().r_md as f32)
                .min_size(egui::vec2(38.0, row_height_default()))
            ).clicked();
            if (send_clicked || (input.lost_focus && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                && !watchlist.discord.input.trim().is_empty()
            {
                let content = watchlist.discord.input.trim().to_string();
                if let Some(ref ch_id) = watchlist.discord.selected_channel {
                    crate::discord::send_message_bg(ch_id.clone(), content.clone());
                    watchlist.discord.messages.push(DiscordMessage {
                        author: watchlist.discord.username.clone(),
                        content,
                        timestamp: "sending...".into(),
                        is_own: true,
                        has_chart: false,
                    });
                }
                watchlist.discord.input.clear();
            }
        });
        ui.add_space(gap_xs());
    }
}
