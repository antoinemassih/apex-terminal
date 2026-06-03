//! GuildAvatarGrid — Discord-style server icon strip.
//!
//! Renders a horizontal row of circle/squircle avatars with:
//! - Selection indicator: squircle shape + glow halo + dot below icon
//! - Hover state: reduced rounding + white glow halo + small dot below
//! - Real icon: drawn via `TextureHandle` with tint based on state
//! - Initials fallback: up to 2 initials centered in a colored squircle
//!
//! The caller is responsible for wrapping this in a `ScrollArea::horizontal`.
//!
//! ```ignore
//! GuildAvatarGrid::new(&guild_list, &icon_map)
//!     .selected(watchlist.discord_selected_guild.as_deref())
//!     .on_click(|id| { ... })
//!     .show(ui, theme);
//! ```

use egui::{Color32, Response, TextureHandle, Ui};
use std::collections::HashMap;
use super::theme::ComponentTheme;
use super::Tooltip;
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Tone};

// ── Widget ────────────────────────────────────────────────────────────────────

pub struct GuildEntry<'a> {
    pub id: &'a str,
    pub name: &'a str,
}

#[must_use = "GuildAvatarGrid does nothing until `.show(ui, theme)` is called"]
pub struct GuildAvatarGrid<'a> {
    guilds: &'a [GuildEntry<'a>],
    icons: &'a HashMap<String, TextureHandle>,
    selected: Option<&'a str>,
    on_click: Option<Box<dyn FnMut(&str) + 'a>>,
    on_hover_name: bool,
}

impl<'a> GuildAvatarGrid<'a> {
    pub fn new(guilds: &'a [GuildEntry<'a>], icons: &'a HashMap<String, TextureHandle>) -> Self {
        Self {
            guilds,
            icons,
            selected: None,
            on_click: None,
            on_hover_name: true,
        }
    }

    pub fn selected(mut self, id: Option<&'a str>) -> Self { self.selected = id; self }
    pub fn on_click<F: FnMut(&str) + 'a>(mut self, f: F) -> Self { self.on_click = Some(Box::new(f)); self }

    /// Render all guild avatars inline (no scroll — caller must wrap in ScrollArea).
    /// Returns a combined Response (last allocated rect; click info is handled via callback).
    pub fn show(mut self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let discord_blurple = st::discord_blurple();
        let icon_size = 32.0_f32;

        // We'll collect the last response to return
        let mut last_resp: Option<Response> = None;

        for guild in self.guilds {
            let selected = self.selected == Some(guild.id);

            let (rect, resp) = ui.allocate_exact_size(
                egui::vec2(icon_size + 6.0, icon_size + 8.0),
                egui::Sense::click());
            let hovered = resp.hovered();

            let icon_rect = egui::Rect::from_center_size(
                egui::pos2(rect.center().x, rect.center().y - 1.0),
                egui::vec2(icon_size, icon_size),
            );

            // Rounding: selected → squircle, hovered → less round, default → circle
            let rounding = if selected { 10.0_f32 } else if hovered { 12.0 } else { 16.0 };

            // Background glow on hover/selected
            if selected || hovered {
                let glow_rect = icon_rect.expand(2.0);
                // Hover bg is always the Discord dark-gray; contrast_fg returns WHITE on
                // dark themes and adapts automatically if bg ever changes. Using
                // contrast_fg instead of hardcoded WHITE keeps the glow visible even
                // when the outer app bg is light (Bauhaus / Ivory / Newsprint).
                let icon_bg = Color32::from_gray(50);
                let glow_color = if selected {
                    st::color_alpha(discord_blurple, st::alpha_strong())
                } else {
                    st::color_alpha(st::contrast_fg(icon_bg), st::alpha_soft())
                };
                ui.painter().rect_filled(glow_rect, rounding + 2.0, glow_color);
            }

            if let Some(tex) = self.icons.get(guild.id) {
                // Real icon
                let bg = if hovered && !selected { Color32::from_gray(50) } else { Color32::from_gray(35) };
                ui.painter().rect_filled(icon_rect, rounding, bg);
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                // Icon bg is always Discord dark-gray; contrast_fg returns WHITE here
                // (same as the original hardcoded value) but is correct by semantics.
                // Idle icon tint follows theme so the icons read correctly on
                // pink/lime/cyan palettes (was hardcoded gray-200, which on a
                // dark-green or dark-purple theme could disappear).
                let tint = if hovered || selected { st::contrast_fg(bg) } else { theme.icon() };
                ui.painter().image(tex.id(), icon_rect, uv, tint);
            } else {
                // Initials fallback
                let bg = if selected { discord_blurple }
                         else if hovered { Color32::from_gray(70) }
                         else { Color32::from_gray(50) };
                ui.painter().rect_filled(icon_rect, rounding, bg);
                let abbrev: String = guild.name.split_whitespace()
                    .filter_map(|w| w.chars().next())
                    .take(2)
                    .collect::<String>()
                    .to_uppercase();
                let font = egui::FontId::monospace(if abbrev.len() > 1 { 9.0 } else { 11.0 });
                // Initials bg is always dark (blurple/gray-70/gray-50); contrast_fg
                // returns WHITE, same as before, but correct if bg ever changes.
                // Idle initials text follows theme.text(); was hardcoded gray-180.
                let text_col = if selected || hovered { st::contrast_fg(bg) } else { palette_ct(theme).base(Tone::Text) };
                ui.painter().text(icon_rect.center(), egui::Align2::CENTER_CENTER, &abbrev, font, text_col);
            }

            // Selection/hover dot under icon
            if selected {
                let dot_center = egui::pos2(icon_rect.center().x, icon_rect.bottom() + 4.0);
                ui.painter().circle_filled(dot_center, 2.5, discord_blurple);
            } else if hovered {
                let dot_center = egui::pos2(icon_rect.center().x, icon_rect.bottom() + 4.0);
                // Hover dot follows theme.dim() so it scales with palette;
                // was hardcoded gray-120.
                ui.painter().circle_filled(dot_center, 1.5, palette_ct(theme).base(Tone::Dim));
            }

            if resp.clicked() {
                if let Some(cb) = self.on_click.as_mut() { cb(guild.id); }
            }

            if self.on_hover_name {
                Tooltip::new(guild.name).show(ui, &resp, theme);
            }
            last_resp = Some(resp);
        }

        // Return a dummy response if no guilds were rendered
        last_resp.unwrap_or_else(|| {
            ui.allocate_exact_size(egui::vec2(0.0, 0.0), egui::Sense::hover()).1
        })
    }
}
