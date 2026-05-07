//! Breadcrumb — path-style nav crumbs separated by chevrons or slashes.
//!
//! API:
//!   let crumbs = ["Watchlists", "Tech", "AAPL"];
//!   let r = Breadcrumb::new(&crumbs).show(ui, theme);
//!   if let Some(idx) = r.clicked_index { navigate_to(idx); }
//!
//!   Breadcrumb::with_items(&[
//!       BreadcrumbItem::new("Home").icon(Icon::HOUSE),
//!       BreadcrumbItem::new("Settings").icon(Icon::GEAR),
//!       BreadcrumbItem::new("Hotkeys"),
//!   ]).separator(BreadcrumbSep::Chevron).show(ui, theme);

use egui::{Response, Ui};

use super::button::Button;
use super::label::Label;
use super::theme::ComponentTheme;
use super::tokens::{Size, Variant};
use crate::chart::renderer::ui::style as st;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BreadcrumbSep {
    #[default] Chevron, // ›
    Slash,              // /
    Dot,                // ·
}

impl BreadcrumbSep {
    fn glyph(&self) -> &'static str {
        match self {
            BreadcrumbSep::Chevron => "\u{203A}",
            BreadcrumbSep::Slash => "/",
            BreadcrumbSep::Dot => "\u{00B7}",
        }
    }
}

#[derive(Clone, Copy)]
pub struct BreadcrumbItem<'a> {
    pub label: &'a str,
    pub icon: Option<&'static str>,
}

impl<'a> BreadcrumbItem<'a> {
    pub fn new(label: &'a str) -> Self { Self { label, icon: None } }
    pub fn icon(mut self, icon: &'static str) -> Self { self.icon = Some(icon); self }
}

#[must_use = "Breadcrumb does nothing until `.show(ui, theme)` is called"]
pub struct Breadcrumb<'a> {
    items: Vec<BreadcrumbItem<'a>>,
    separator: BreadcrumbSep,
    size: Size,
}

pub struct BreadcrumbResponse {
    pub response: Response,
    pub clicked_index: Option<usize>,
}

impl<'a> Breadcrumb<'a> {
    pub fn new(crumbs: &'a [&'a str]) -> Self {
        Self {
            items: crumbs.iter().map(|s| BreadcrumbItem::new(s)).collect(),
            separator: BreadcrumbSep::default(),
            size: Size::Sm,
        }
    }

    pub fn with_items(items: &'a [BreadcrumbItem<'a>]) -> Self {
        Self {
            items: items.to_vec(),
            separator: BreadcrumbSep::default(),
            size: Size::Sm,
        }
    }

    pub fn separator(mut self, s: BreadcrumbSep) -> Self { self.separator = s; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> BreadcrumbResponse {
        let Breadcrumb { items, separator, size } = self;
        let last_idx = items.len().saturating_sub(1);
        let mut clicked_index = None;

        let resp = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = st::gap_2xs();
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    let _ = Label::new(separator.glyph()).size(size).muted().show(ui, theme);
                }

                if i == last_idx {
                    // Final crumb: plain text, current location.
                    let _ = Label::new(item.label).size(size).show(ui, theme);
                } else {
                    let mut btn = Button::new(item.label).variant(Variant::Link).size(size);
                    if let Some(ic) = item.icon {
                        btn = btn.leading_icon(ic);
                    }
                    if btn.show(ui, theme).clicked() {
                        clicked_index = Some(i);
                    }
                }
            }
        });

        BreadcrumbResponse { response: resp.response, clicked_index }
    }
}
