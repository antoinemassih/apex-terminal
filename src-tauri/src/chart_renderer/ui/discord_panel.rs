//! Discord chat integration panel.

use egui;
use super::style::*;
use super::widgets;
use super::super::gpu::{Watchlist, DiscordMessage, Theme};
use crate::ui_kit::icons::Icon;

const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }

/// Drain background Discord results (textures, messages, guilds). Call before rendering.
pub(crate) fn drain_background(ctx: &egui::Context, watchlist: &mut Watchlist) {
    // Check auth
    if !watchlist.discord_authenticated {
        if let Some(auth) = crate::discord::get_auth() {
            watchlist.discord_authenticated = true;
            watchlist.discord_username = auth.username.clone();
            watchlist.discord_user_id = auth.user_id.clone();
            watchlist.discord_connecting = false;
            crate::discord::fetch_guilds_bg();
        }
    }
    // Drain guilds
    if let Some(guilds) = crate::discord::drain_guilds() {
        watchlist.discord_guilds = guilds;
    }
    // Drain guild icons → create textures
    for icon in crate::discord::drain_icons() {
        let pixels: Vec<egui::Color32> = icon.rgba.chunks_exact(4)
            .map(|c| egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
            .collect();
        let img = egui::ColorImage { size: [icon.width as usize, icon.height as usize], pixels };
        let tex = ctx.load_texture(format!("guild_{}", icon.guild_id), img, egui::TextureOptions::LINEAR);
        watchlist.discord_guild_icons.insert(icon.guild_id, tex);
    }
    // Drain channels
    if let Some(channels) = crate::discord::drain_channels() {
        watchlist.discord_channels = channels;
        watchlist.discord_channels_loading = false;
    }
    // Drain messages
    if let Some((msgs, is_append)) = crate::discord::drain_messages() {
        if is_append {
            for m in &msgs {
                let is_own = m.author.id == watchlist.discord_user_id;
                watchlist.discord_messages.push(DiscordMessage {
                    author: m.author.display_name().to_string(),
                    content: m.content.clone(),
                    timestamp: crate::discord::relative_time(&m.timestamp),
                    is_own,
                    has_chart: false,
                });
                watchlist.discord_last_msg_id = Some(m.id.clone());
            }
        } else {
            watchlist.discord_messages.clear();
            for m in &msgs {
                let is_own = m.author.id == watchlist.discord_user_id;
                watchlist.discord_messages.push(DiscordMessage {
                    author: m.author.display_name().to_string(),
                    content: m.content.clone(),
                    timestamp: crate::discord::relative_time(&m.timestamp),
                    is_own,
                    has_chart: false,
                });
            }
            watchlist.discord_last_msg_id = msgs.last().map(|m| m.id.clone());
            watchlist.discord_messages_loading = false;
        }
    }
    // Drain send result
    if let Some(result) = crate::discord::drain_send() {
        if let Ok(msg) = result {
            watchlist.discord_messages.push(DiscordMessage {
                author: msg.author.display_name().to_string(),
                content: msg.content.clone(),
                timestamp: "now".into(),
                is_own: msg.author.id == watchlist.discord_user_id,
                has_chart: false,
            });
            watchlist.discord_last_msg_id = Some(msg.id);
        }
    }
    // Poll for new messages every 5s (only after initial load completes)
    if watchlist.discord_selected_channel.is_some()
        && !watchlist.discord_messages_loading
        && watchlist.discord_last_msg_id.is_some()
    {
        let should_poll = watchlist.discord_poll_timer
            .map(|t| t.elapsed().as_secs_f32() > 5.0)
            .unwrap_or(false);
        if should_poll {
            if let Some(ref ch_id) = watchlist.discord_selected_channel {
                crate::discord::fetch_messages_bg(ch_id.clone(), watchlist.discord_last_msg_id.clone());
                watchlist.discord_poll_timer = Some(std::time::Instant::now());
            }
        }
    }
}

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
// ── Discord Chat side panel ─────────────────────────────────────────────────
if watchlist.discord_open {
    drain_background(ctx, watchlist);
}

if watchlist.discord_open {
    egui::SidePanel::left("discord_chat")
        .default_width(260.0)
        .min_width(200.0)
        .max_width(400.0)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
            .stroke(egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_STRONG))))
        .show(ctx, |ui| {
            draw_content(ui, watchlist, t);
        });
}

}

/// Tab body content (no SidePanel wrapper). Used by feed_panel Discord tab.
/// NOTE: caller must call `drain_background(ctx, watchlist)` before this each frame.
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    let discord_blurple = rgb(88, 101, 242);
    let panel_w = ui.available_width();
    {
            if !watchlist.discord_authenticated {
                // ── Not authenticated: Connect button ──
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.add(widgets::text::SectionLabel::new("DISCORD").color(discord_blurple));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(6.0);
                        if close_button(ui, t.dim) { watchlist.discord_open = false; }
                    });
                });
                let avail = ui.available_size();
                ui.allocate_ui_with_layout(
                    egui::vec2(panel_w, avail.y),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.add_space(avail.y * 0.25);
                        ui.label(egui::RichText::new(Icon::CHAT_DOTS).size(36.0).color(discord_blurple.gamma_multiply(0.5)));
                        ui.add_space(10.0);
                        if !crate::discord::is_configured() {
                            ui.add(widgets::text::MonospaceCode::new("Discord not configured").xs().color(t.dim));
                            ui.add(widgets::text::MonospaceCode::new("Add discord.env with credentials").xs().color(t.dim.gamma_multiply(0.5)));
                        } else if watchlist.discord_connecting {
                            ui.add(widgets::text::MonospaceCode::new("Waiting for authorization...").xs().color(t.dim));
                            ui.add_space(6.0);
                            super::chart_widgets::refined_spinner(ui, t.accent);
                            ui.add_space(4.0);
                            ui.add(widgets::text::MonospaceCode::new("Complete sign-in in your browser").xs().color(t.dim.gamma_multiply(0.5)));
                        } else {
                            if ui.add(egui::Button::new(
                                egui::RichText::new("  Connect Discord  ").monospace().size(10.0).strong().color(egui::Color32::WHITE))
                                .fill(discord_blurple)
                                .corner_radius(RADIUS_LG)
                                .min_size(egui::vec2(180.0, 36.0))
                            ).clicked() {
                                watchlist.discord_connecting = true;
                                crate::discord::start_oauth2();
                            }
                            ui.add_space(8.0);
                            ui.add(widgets::text::MonospaceCode::new("Chat with your trading community").xs().color(t.dim.gamma_multiply(0.5)));
                        }
                    },
                );
            } else {
                // ── Authenticated: full Discord UI ──
                let has_guilds = !watchlist.discord_guilds.is_empty();

                // ── Header ──
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    if !watchlist.discord_channel.is_empty() {
                        ui.add(widgets::text::BodyLabel::new(&watchlist.discord_channel).monospace(true).strong(true).size(10.0).color(egui::Color32::WHITE));
                    } else {
                        ui.add(widgets::text::SectionLabel::new("DISCORD").color(discord_blurple));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(6.0);
                        if close_button(ui, t.dim) { watchlist.discord_open = false; }
                        if ui.add(egui::Button::new(
                            egui::RichText::new("×").monospace().size(9.0).color(rgb(231, 76, 60)))
                            .fill(egui::Color32::TRANSPARENT).frame(false)
                        ).on_hover_text("Disconnect").clicked() {
                            crate::discord::disconnect();
                            watchlist.discord_authenticated = false;
                            watchlist.discord_username.clear();
                            watchlist.discord_user_id.clear();
                            watchlist.discord_guilds.clear();
                            watchlist.discord_selected_guild = None;
                            watchlist.discord_channels.clear();
                            watchlist.discord_selected_channel = None;
                            watchlist.discord_channel.clear();
                            watchlist.discord_messages.clear();
                            watchlist.discord_guild_icons.clear();
                            watchlist.discord_last_msg_id = None;
                            watchlist.discord_poll_timer = None;
                        }
                    });
                });
                ui.add_space(4.0);

                // ── Server strip (horizontal, top) ──
                if has_guilds {
                    // Dark strip background
                    let strip_h = 44.0;
                    let (strip_bg_rect, _) = ui.allocate_exact_size(egui::vec2(panel_w, 0.0), egui::Sense::hover());
                    let bg_rect = egui::Rect::from_min_size(strip_bg_rect.min, egui::vec2(panel_w, strip_h));
                    ui.painter().rect_filled(bg_rect, 0.0, color_alpha(egui::Color32::BLACK, ALPHA_TINT));

                    egui::ScrollArea::horizontal().id_salt("guild_strip").show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(6.0);
                            let guild_list: Vec<_> = watchlist.discord_guilds.clone();
                            for guild in &guild_list {
                                let selected = watchlist.discord_selected_guild.as_ref() == Some(&guild.id);
                                let icon_size = 32.0;

                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(icon_size + 6.0, icon_size + 8.0), egui::Sense::click());
                                let hovered = resp.hovered();

                                let icon_rect = egui::Rect::from_center_size(
                                    egui::pos2(rect.center().x, rect.center().y - 1.0),
                                    egui::vec2(icon_size, icon_size),
                                );

                                // Rounding: selected → squircle, hovered → less round, default → circle
                                let rounding = if selected { 10.0 } else if hovered { 12.0 } else { 16.0 };

                                // Background glow on hover/selected
                                if selected || hovered {
                                    let glow_rect = icon_rect.expand(2.0);
                                    let glow_color = if selected { color_alpha(discord_blurple, ALPHA_STRONG) } else { color_alpha(egui::Color32::WHITE, ALPHA_SOFT) };
                                    ui.painter().rect_filled(glow_rect, rounding + 2.0, glow_color);
                                }

                                if let Some(tex) = watchlist.discord_guild_icons.get(&guild.id) {
                                    // Real icon
                                    let bg = if hovered && !selected { egui::Color32::from_gray(50) } else { egui::Color32::from_gray(35) };
                                    ui.painter().rect_filled(icon_rect, rounding, bg);
                                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                                    let tint = if hovered || selected { egui::Color32::WHITE } else { egui::Color32::from_gray(200) };
                                    ui.painter().image(tex.id(), icon_rect, uv, tint);
                                } else {
                                    // Initials fallback
                                    let bg = if selected { discord_blurple }
                                             else if hovered { egui::Color32::from_gray(70) }
                                             else { egui::Color32::from_gray(50) };
                                    ui.painter().rect_filled(icon_rect, rounding, bg);
                                    let abbrev: String = guild.name.split_whitespace()
                                        .filter_map(|w| w.chars().next())
                                        .take(2)
                                        .collect::<String>()
                                        .to_uppercase();
                                    let font = egui::FontId::monospace(if abbrev.len() > 1 { 9.0 } else { 11.0 });
                                    let text_col = if selected || hovered { egui::Color32::WHITE } else { egui::Color32::from_gray(180) };
                                    ui.painter().text(icon_rect.center(), egui::Align2::CENTER_CENTER, &abbrev, font, text_col);
                                }

                                // Selection dot under icon
                                if selected {
                                    let dot_center = egui::pos2(icon_rect.center().x, icon_rect.bottom() + 4.0);
                                    ui.painter().circle_filled(dot_center, 2.5, discord_blurple);
                                } else if hovered {
                                    let dot_center = egui::pos2(icon_rect.center().x, icon_rect.bottom() + 4.0);
                                    ui.painter().circle_filled(dot_center, 1.5, egui::Color32::from_gray(120));
                                }

                                if resp.clicked() {
                                    watchlist.discord_selected_guild = Some(guild.id.clone());
                                    watchlist.discord_channels.clear();
                                    watchlist.discord_selected_channel = None;
                                    watchlist.discord_messages.clear();
                                    watchlist.discord_last_msg_id = None;
                                    watchlist.discord_channel.clear();
                                    watchlist.discord_channels_loading = true;
                                    crate::discord::fetch_channels_bg(guild.id.clone());
                                }
                                resp.on_hover_text(&guild.name);
                            }
                            ui.add_space(4.0);
                        });
                    });
                    ui.add_space(2.0);
                    // Separator
                    let sep_rect = ui.allocate_exact_size(egui::vec2(panel_w, 1.0), egui::Sense::hover()).0;
                    ui.painter().rect_filled(sep_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_DIM));
                }

                // ── Content area ──
                {
                    let content_w = panel_w;

                        if watchlist.discord_selected_guild.is_none() {
                            // No server selected
                            let avail = ui.available_size();
                            ui.allocate_ui_with_layout(
                                egui::vec2(content_w, avail.y),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    ui.add_space(avail.y * 0.3);
                                    ui.add(widgets::text::MonospaceCode::new("Select a server").sm().color(t.dim.gamma_multiply(0.6)));
                                    ui.add(widgets::text::MonospaceCode::new("from the icons above").xs().color(t.dim.gamma_multiply(0.4)));
                                },
                            );
                        } else if watchlist.discord_selected_channel.is_none() {
                            // Server selected, show channel list
                            ui.add_space(4.0);

                            if watchlist.discord_channels_loading {
                                ui.horizontal(|ui| {
                                    ui.add_space(6.0);
                                    super::chart_widgets::refined_spinner(ui, t.accent);
                                    ui.add(widgets::text::MonospaceCode::new("Loading channels...").xs().color(t.dim));
                                });
                            } else if watchlist.discord_channels.is_empty() {
                                // Bot not in this server — show invite button
                                let avail = ui.available_size();
                                ui.allocate_ui_with_layout(
                                    egui::vec2(content_w, avail.y),
                                    egui::Layout::top_down(egui::Align::Center),
                                    |ui| {
                                        ui.add_space(avail.y * 0.2);
                                        ui.label(egui::RichText::new(Icon::PLUGS_CONNECTED).size(28.0).color(t.dim.gamma_multiply(0.5)));
                                        ui.add_space(6.0);
                                        ui.add(widgets::text::MonospaceCode::new("Bot not in this server").xs().color(t.dim));
                                        ui.add_space(4.0);
                                        ui.add(widgets::text::MonospaceCode::new("Add the Apex bot to enable\nchannels & messaging").xs().color(t.dim.gamma_multiply(0.5)));
                                        ui.add_space(8.0);
                                        if ui.add(egui::Button::new(
                                            egui::RichText::new("  Add Bot to Server  ").monospace().size(9.0).strong().color(egui::Color32::WHITE))
                                            .fill(discord_blurple)
                                            .corner_radius(4.0)
                                            .min_size(egui::vec2(160.0, 30.0))
                                        ).clicked() {
                                            let guild_id = watchlist.discord_selected_guild.as_deref().unwrap_or("");
                                            let url = format!(
                                                "https://discord.com/oauth2/authorize?client_id=1492118514482417776&scope=bot&permissions=68608&guild_id={}",
                                                guild_id
                                            );
                                            let _ = open::that(&url);
                                        }
                                        ui.add_space(8.0);
                                        ui.add(widgets::text::CaptionLabel::new("Server admins can also\nadd the bot themselves").color(t.dim).gamma(0.4));
                                        ui.add_space(10.0);
                                        if ui.add(egui::Button::new(
                                            egui::RichText::new("Retry").monospace().size(8.0).color(t.dim))
                                            .fill(color_alpha(t.toolbar_border, ALPHA_TINT))
                                            .corner_radius(RADIUS_MD)
                                        ).clicked() {
                                            if let Some(ref gid) = watchlist.discord_selected_guild {
                                                watchlist.discord_channels_loading = true;
                                                crate::discord::fetch_channels_bg(gid.clone());
                                            }
                                        }
                                    },
                                );
                            } else {
                                // Channel list
                                egui::ScrollArea::vertical().id_salt("discord_channels").show(ui, |ui| {
                                    ui.set_min_width(content_w - 4.0);
                                    let channels: Vec<_> = watchlist.discord_channels.clone();
                                    // Group by category
                                    let mut current_category = String::new();
                                    for ch in &channels {
                                        if ch.is_category() {
                                            // Category header
                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.add_space(8.0);
                                                let name = ch.name.as_deref().unwrap_or("UNKNOWN").to_uppercase();
                                                ui.label(egui::RichText::new(Icon::CARET_DOWN).size(8.0).color(t.dim.gamma_multiply(0.5)));
                                                ui.add(widgets::text::SectionLabel::new(&name).xs().color(t.dim).gamma(0.6));
                                            });
                                            current_category = ch.id.clone();
                                            ui.add_space(2.0);
                                        } else if ch.is_text() {
                                            let name = ch.name.as_deref().unwrap_or("unknown");
                                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(content_w, 22.0), egui::Sense::click());
                                            let hovered = resp.hovered();
                                            if hovered {
                                                ui.painter().rect_filled(rect, 3.0, color_alpha(egui::Color32::WHITE, 8));
                                            }
                                            let text_color = if hovered { egui::Color32::WHITE } else { egui::Color32::from_gray(160) };
                                            let font = egui::FontId::monospace(9.5);
                                            ui.painter().text(
                                                egui::pos2(rect.left() + 14.0, rect.center().y),
                                                egui::Align2::LEFT_CENTER,
                                                format!("# {}", name),
                                                font,
                                                text_color,
                                            );
                                            if resp.clicked() {
                                                watchlist.discord_selected_channel = Some(ch.id.clone());
                                                watchlist.discord_channel = format!("# {}", name);
                                                watchlist.discord_messages.clear();
                                                watchlist.discord_last_msg_id = None;
                                                watchlist.discord_messages_loading = true;
                                                watchlist.discord_poll_timer = None;
                                                crate::discord::fetch_messages_bg(ch.id.clone(), None);
                                            }
                                        }
                                    }
                                });
                            }
                        } else {
                            // ── Channel selected: show messages + input ──
                            ui.add_space(2.0);

                            // Back button + channel name
                            ui.horizontal(|ui| {
                                ui.add_space(6.0);
                                if ui.add(egui::Button::new(
                                    egui::RichText::new(Icon::CARET_RIGHT).size(9.0).color(t.dim))
                                    .fill(egui::Color32::TRANSPARENT).frame(false)
                                ).on_hover_text("Back to channels").clicked() {
                                    watchlist.discord_selected_channel = None;
                                    watchlist.discord_messages.clear();
                                    watchlist.discord_last_msg_id = None;
                                    watchlist.discord_poll_timer = None;
                                }
                                ui.add(widgets::text::MonospaceCode::new(&watchlist.discord_channel).xs().color(t.dim));
                            });
                            ui.add_space(2.0);

                            let author_colors: &[egui::Color32] = &[
                                rgb(74, 158, 255), rgb(46, 204, 113), rgb(243, 156, 18),
                                rgb(155, 89, 182), rgb(231, 76, 60), rgb(26, 188, 156),
                                rgb(241, 196, 15), rgb(52, 152, 219),
                            ];

                            // Messages
                            let input_h = 36.0;
                            let msg_area_h = ui.available_height() - input_h;
                            egui::ScrollArea::vertical()
                                .id_salt("discord_msgs")
                                .max_height(msg_area_h.max(60.0))
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    ui.set_min_width(content_w - 4.0);
                                    if watchlist.discord_messages_loading {
                                        ui.add_space(20.0);
                                        ui.horizontal(|ui| {
                                            ui.add_space(8.0);
                                            super::chart_widgets::refined_spinner(ui, t.accent);
                                            ui.add(widgets::text::MonospaceCode::new("Loading messages...").xs().color(t.dim));
                                        });
                                    } else if watchlist.discord_messages.is_empty() {
                                        ui.add_space(20.0);
                                        ui.horizontal(|ui| {
                                            ui.add_space(6.0);
                                            ui.add(widgets::text::MonospaceCode::new("No messages in this channel").xs().color(t.dim).gamma(0.5));
                                        });
                                    }
                                    let mut prev_author = String::new();
                                    for msg in &watchlist.discord_messages {
                                        if msg.content.is_empty() { continue; }
                                        let author_hash = msg.author.bytes().fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
                                        let author_col = author_colors[author_hash % author_colors.len()];
                                        let same_author = msg.author == prev_author;

                                        ui.horizontal(|ui| {
                                            ui.add_space(8.0);
                                            ui.vertical(|ui| {
                                                if !same_author {
                                                    if !prev_author.is_empty() { ui.add_space(4.0); }
                                                    ui.horizontal(|ui| {
                                                        // Author avatar circle
                                                        let (av_rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                                                        let initial = msg.author.chars().next().unwrap_or('?').to_uppercase().to_string();
                                                        ui.painter().circle_filled(av_rect.center(), 9.0, color_alpha(author_col, ALPHA_DIM));
                                                        ui.painter().text(av_rect.center(), egui::Align2::CENTER_CENTER, &initial, egui::FontId::monospace(8.0), egui::Color32::WHITE);

                                                        ui.add(widgets::text::MonospaceCode::new(&msg.author).xs().strong(true).color(author_col));
                                                        ui.add(widgets::text::CaptionLabel::new(&msg.timestamp).color(t.dim).gamma(0.4));
                                                    });
                                                }
                                                // Message content with left indent
                                                ui.horizontal(|ui| {
                                                    ui.add_space(22.0); // align with text after avatar
                                                    ui.add(egui::Label::new(
                                                        egui::RichText::new(&msg.content).monospace().size(9.0).color(egui::Color32::from_gray(210))
                                                    ).wrap_mode(egui::TextWrapMode::Wrap));
                                                });
                                            });
                                        });
                                        prev_author = msg.author.clone();
                                    }
                                    ui.add_space(4.0);
                                });

                            // Input area
                            let sep_rect = ui.allocate_exact_size(egui::vec2(content_w, 1.0), egui::Sense::hover()).0;
                            ui.painter().rect_filled(sep_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(6.0);
                                let input = ui.add(
                                    egui::TextEdit::singleline(&mut watchlist.discord_input)
                                        .desired_width(content_w - 60.0)
                                        .font(egui::TextStyle::Monospace)
                                        .text_color(egui::Color32::from_gray(220))
                                        .hint_text(format!("Message {}...", watchlist.discord_channel))
                                );
                                let send_clicked = ui.add(egui::Button::new(
                                    egui::RichText::new("Send").monospace().size(9.0).color(egui::Color32::WHITE))
                                    .fill(discord_blurple)
                                    .corner_radius(RADIUS_MD)
                                    .min_size(egui::vec2(38.0, 22.0))
                                ).clicked();
                                if (send_clicked || (input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                                    && !watchlist.discord_input.trim().is_empty()
                                {
                                    let content = watchlist.discord_input.trim().to_string();
                                    if let Some(ref ch_id) = watchlist.discord_selected_channel {
                                        crate::discord::send_message_bg(ch_id.clone(), content.clone());
                                        // Optimistic insert
                                        watchlist.discord_messages.push(DiscordMessage {
                                            author: watchlist.discord_username.clone(),
                                            content,
                                            timestamp: "sending...".into(),
                                            is_own: true,
                                            has_chart: false,
                                        });
                                    }
                                    watchlist.discord_input.clear();
                                }
                            });
                            ui.add_space(4.0);
                        }
                }
            }
    }
}
