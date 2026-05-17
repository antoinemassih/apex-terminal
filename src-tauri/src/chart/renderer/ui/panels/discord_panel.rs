//! Discord chat integration panel.

use egui;
use super::super::style::*;
use super::super::widgets;
use super::super::widgets::inputs::TextInput;
use super::super::widgets::headers::PanelHeaderWithClose;
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::{Variant, Size};
use crate::ui_kit::widgets::{GuildAvatarGrid, GuildEntry};
use super::super::super::gpu::{Watchlist, DiscordMessage, Theme};
use crate::ui_kit::icons::Icon;

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
        // Edge-only border: SidePanel resize-handle provides the divider.
        .frame(egui::Frame::NONE.fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 }))
        .show(ctx, |ui| {
            draw_content(ui, watchlist, t);
        });
}

}

/// Tab body content (no SidePanel wrapper). Used by feed_panel Discord tab.
/// NOTE: caller must call `drain_background(ctx, watchlist)` before this each frame.
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    // Brand color from the semantic-color tokens (style::discord_blurple()).
    // Bound through the explicit module path because the function name is
    // shadowed by the local binding within this scope.
    let discord_blurple = super::super::style::discord_blurple();
    let panel_w = ui.available_width();
    {
            if !watchlist.discord_authenticated {
                // ── Not authenticated: Connect button ──
                ui.add_space(8.0);
                if PanelHeaderWithClose::new("DISCORD").accent(discord_blurple).dim(t.dim).watchlist(watchlist).show(ui) {
                    watchlist.discord_open = false;
                }
                let avail = ui.available_size();
                ui.allocate_ui_with_layout(
                    egui::vec2(panel_w, avail.y),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.add_space(avail.y * 0.25);
                        ui.label(egui::RichText::new(Icon::CHAT_DOTS).size(32.0).color(color_half(discord_blurple)));
                        ui.add_space(12.0);
                        if !crate::discord::is_configured() {
                            ui.add(widgets::text::MonospaceCode::new("Discord not configured").xs().color(t.dim));
                            ui.add(widgets::text::MonospaceCode::new("Add discord.env with credentials").xs().color(color_half(t.dim)));
                        } else if watchlist.discord_connecting {
                            ui.add(widgets::text::MonospaceCode::new("Waiting for authorization...").xs().color(t.dim));
                            ui.add_space(8.0);
                            crate::ui_kit::widgets::Spinner::new().show(ui, t);
                            ui.add_space(4.0);
                            ui.add(widgets::text::MonospaceCode::new("Complete sign-in in your browser").xs().color(color_half(t.dim)));
                        } else {
                            // WHITE foreground is intentional brand contrast on the Discord blurple fill.
                            if ui.add(Button::new("  Connect Discord  ")
                                .variant(Variant::Chrome)
                                .size(Size::Sm)
                                .fg(egui::Color32::WHITE)
                                .fill(discord_blurple)
                                .corner_radius(current().r_lg as f32)
                                .min_size(egui::vec2(180.0, 36.0))
                            ).clicked() {
                                watchlist.discord_connecting = true;
                                crate::discord::start_oauth2();
                            }
                            ui.add_space(8.0);
                            ui.add(widgets::text::MonospaceCode::new("Chat with your trading community").xs().color(color_half(t.dim)));
                        }
                    },
                );
            } else {
                // ── Authenticated: full Discord UI ──
                let has_guilds = !watchlist.discord_guilds.is_empty();

                // ── Header ──
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    if !watchlist.discord_channel.is_empty() {
                        ui.add(widgets::text::BodyLabel::new(&watchlist.discord_channel).monospace(true).strong(true).size(font_sm()).color(t.text));
                    } else {
                        ui.add(widgets::text::SectionLabel::new("DISCORD").color(discord_blurple));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(8.0);
                        if Button::close().show(ui, t).clicked() { watchlist.discord_open = false; }
                        if ui.add(Button::new("×")
                            .variant(Variant::TextOnly)
                            .size(Size::Sm)
                            .fg(t.bear)
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
                    // Was Color32::BLACK + alpha_tint — broke 4 light themes.
                    // Use the theme-aware shadow color instead so the strip still reads
                    // as "darker than chart bg" on dark themes and "darker than cream
                    // bg" on light themes.
                    ui.painter().rect_filled(bg_rect, 0.0, shadow_color_alpha(t, alpha_tint()));

                    egui::ScrollArea::horizontal().id_salt("guild_strip").show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            let guild_entries: Vec<GuildEntry<'_>> = watchlist.discord_guilds.iter()
                                .map(|g| GuildEntry { id: &g.id, name: &g.name })
                                .collect();
                            let mut clicked_guild: Option<String> = None;
                            GuildAvatarGrid::new(&guild_entries, &watchlist.discord_guild_icons)
                                .selected(watchlist.discord_selected_guild.as_deref())
                                .on_click(|id| { clicked_guild = Some(id.to_string()); })
                                .show(ui, t);
                            if let Some(id) = clicked_guild {
                                watchlist.discord_selected_guild = Some(id.clone());
                                watchlist.discord_channels.clear();
                                watchlist.discord_selected_channel = None;
                                watchlist.discord_messages.clear();
                                watchlist.discord_last_msg_id = None;
                                watchlist.discord_channel.clear();
                                watchlist.discord_channels_loading = true;
                                crate::discord::fetch_channels_bg(id);
                            }
                            ui.add_space(4.0);
                        });
                    });
                    ui.add_space(4.0);
                    // Separator
                    let sep_rect = ui.allocate_exact_size(egui::vec2(panel_w, 1.0), egui::Sense::hover()).0;
                    ui.painter().rect_filled(sep_rect, 0.0, color_alpha(t.toolbar_border, alpha_dim()));
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
                                    ui.add(widgets::text::MonospaceCode::new("Select a server").sm().color(color_muted(t.dim)));
                                    ui.add(widgets::text::MonospaceCode::new("from the icons above").xs().color(color_dim(t.dim)));
                                },
                            );
                        } else if watchlist.discord_selected_channel.is_none() {
                            // Server selected, show channel list
                            ui.add_space(4.0);

                            if watchlist.discord_channels_loading {
                                ui.horizontal(|ui| {
                                    ui.add_space(8.0);
                                    crate::ui_kit::widgets::Spinner::new().show(ui, t);
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
                                        ui.label(egui::RichText::new(Icon::PLUGS_CONNECTED).size(32.0).color(color_half(t.dim)));
                                        ui.add_space(8.0);
                                        ui.add(widgets::text::MonospaceCode::new("Bot not in this server").xs().color(t.dim));
                                        ui.add_space(4.0);
                                        ui.add(widgets::text::MonospaceCode::new("Add the Apex bot to enable\nchannels & messaging").xs().color(color_half(t.dim)));
                                        ui.add_space(8.0);
                                        // WHITE foreground is intentional brand contrast on the Discord blurple fill.
                                        if ui.add(Button::new("  Add Bot to Server  ")
                                            .variant(Variant::Chrome)
                                            .size(Size::Sm)
                                            .fg(egui::Color32::WHITE)
                                            .fill(discord_blurple)
                                            .corner_radius(current().r_sm as f32)
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
                                        ui.add_space(12.0);
                                        if ui.add(Button::new("Retry")
                                            .variant(Variant::Chrome)
                                            .size(Size::Xs)
                                            .fg(t.dim)
                                            .fill(color_alpha(t.toolbar_border, alpha_tint()))
                                            .corner_radius(current().r_md as f32)
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
                                            // Category header — SectionLabel (already unified)
                                            ui.add_space(8.0);
                                            ui.horizontal(|ui| {
                                                ui.add_space(8.0);
                                                let name = ch.name.as_deref().unwrap_or("UNKNOWN").to_uppercase();
                                                ui.label(egui::RichText::new(Icon::CARET_DOWN).size(font_xs()).color(color_half(t.dim)));
                                                ui.add(widgets::text::SectionLabel::new(&name).xs().color(t.dim).gamma(0.6));
                                            });
                                            current_category = ch.id.clone();
                                            ui.add_space(4.0);
                                        } else if ch.is_text() {
                                            let name = ch.name.as_deref().unwrap_or("unknown").to_string();
                                            // ── ListRow migration: channel row ──
                                            let ch_text = t.dim.gamma_multiply(1.3);
                                            let resp = widgets::rows::ListRow::new(22.0)
                                                .left_icon("#", t.dim)
                                                .body({
                                                    let name = name.clone();
                                                    move |ui| {
                                                        ui.add(widgets::text::MonospaceCode::new(&name).xs().color(ch_text));
                                                    }
                                                })
                                                .theme(t)
                                                .show(ui);
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
                            ui.add_space(4.0);

                            // Back button + channel name
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                if ui.add(Button::new(Icon::CARET_RIGHT)
                                    .variant(Variant::TextOnly)
                                    .size(Size::Sm)
                                    .fg(t.dim)
                                ).on_hover_text("Back to channels").clicked() {
                                    watchlist.discord_selected_channel = None;
                                    watchlist.discord_messages.clear();
                                    watchlist.discord_last_msg_id = None;
                                    watchlist.discord_poll_timer = None;
                                }
                                ui.add(widgets::text::MonospaceCode::new(&watchlist.discord_channel).xs().color(t.dim));
                            });
                            ui.add_space(4.0);

                            // Author-distinction palette is provided by style::chat_author_color
                            // (backed by the ChatAuthorPalette design-token family). Indexed
                            // directly by the author-name hash — no local array needed.

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
                                        ui.add_space(8.0);
                                        let row_w = (content_w - 24.0).max(80.0);
                                        for _ in 0..5 {
                                            ui.add_space(4.0);
                                            ui.horizontal(|ui| {
                                                ui.add_space(8.0);
                                                crate::ui_kit::widgets::Skeleton::circle(20.0).show(ui, t);
                                                ui.add_space(8.0);
                                                ui.vertical(|ui| {
                                                    crate::ui_kit::widgets::Skeleton::text(row_w * 0.35).show(ui, t);
                                                    ui.add_space(2.0);
                                                    crate::ui_kit::widgets::Skeleton::text(row_w * 0.75).show(ui, t);
                                                });
                                            });
                                            ui.add_space(4.0);
                                        }
                                    } else if watchlist.discord_messages.is_empty() {
                                        ui.add_space(20.0);
                                        ui.horizontal(|ui| {
                                            ui.add_space(8.0);
                                            ui.add(widgets::text::MonospaceCode::new("No messages in this channel").xs().color(t.dim).gamma(0.5));
                                        });
                                    }
                                    let mut prev_author = String::new();
                                    for msg in &watchlist.discord_messages {
                                        if msg.content.is_empty() { continue; }
                                        let author_hash = msg.author.bytes().fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
                                        let author_col = chat_author_color(author_hash);
                                        let same_author = msg.author == prev_author;

                                        if !same_author {
                                            if !prev_author.is_empty() { ui.add_space(4.0); }
                                            // ── ListRow migration: author header row ──
                                            // left_painter_circle = avatar dot; body = author + timestamp.
                                            // The initial letter is painted via the dot color; full circle
                                            // with initial requires painter access — kept inline below the row.
                                            let author = msg.author.clone();
                                            let timestamp = msg.timestamp.clone();
                                            let dim = t.dim;
                                            // Use ListRow for the header strip with left dot as avatar color indicator
                                            widgets::rows::ListRow::new(18.0)
                                                .left_painter_circle(author_col)
                                                .hover_enabled(false)
                                                .body(move |ui| {
                                                    ui.add(widgets::text::MonospaceCode::new(&author).xs().strong(true).color(author_col));
                                                    ui.add(widgets::text::CaptionLabel::new(&timestamp).color(dim).gamma(0.4));
                                                })
                                                .theme(t)
                                                .show(ui);
                                        }
                                        // Message content — INLINE EXCEPTION: egui::Label required for
                                        // wrap_mode(Wrap); no widget currently supports wrapping body text.
                                        ui.horizontal(|ui| {
                                            ui.add_space(20.0); // align with text after avatar dot
                                            ui.add(egui::Label::new(
                                                egui::RichText::new(&msg.content).monospace().size(font_sm()).color(t.text)
                                            ).wrap_mode(egui::TextWrapMode::Wrap));
                                        });
                                        prev_author = msg.author.clone();
                                    }
                                    ui.add_space(4.0);
                                });

                            // Input area
                            let sep_rect = ui.allocate_exact_size(egui::vec2(content_w, 1.0), egui::Sense::hover()).0;
                            ui.painter().rect_filled(sep_rect, 0.0, color_alpha(t.toolbar_border, alpha_muted()));
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                let input = TextInput::new(&mut watchlist.discord_input)
                                    .width(content_w - 60.0)
                                    .text_color(t.text)
                                    .placeholder(&format!("Message {}...", watchlist.discord_channel))
                                    .theme(t)
                                    .show(ui);
                                // WHITE foreground is intentional brand contrast on the Discord blurple fill.
                                let send_clicked = ui.add(Button::new("Send")
                                    .variant(Variant::Chrome)
                                    .size(Size::Sm)
                                    .fg(egui::Color32::WHITE)
                                    .fill(discord_blurple)
                                    .corner_radius(current().r_md as f32)
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
