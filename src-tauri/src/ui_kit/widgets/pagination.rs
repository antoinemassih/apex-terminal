//! Pagination — page-jump controls for long lists.
//!
//! API:
//!   let mut page: usize = 0;
//!   ui.add(Pagination::new(&mut page, total_pages));
//!
//!   Pagination::new(&mut page, total_pages)
//!     .compact(true)         // just prev/next, no page numbers
//!     .show_first_last(true) // ⏮ and ⏭ buttons
//!     .show(ui, theme);

use egui::{Response, Ui};

use super::button::Button;
use super::label::Label;
use super::theme::ComponentTheme;
use super::tokens::{Size, Variant};
use crate::chart::renderer::ui::style as st;

#[must_use = "Pagination does nothing until `.show(ui, theme)` is called"]
pub struct Pagination<'a> {
    page: &'a mut usize,
    total_pages: usize,
    compact: bool,
    show_first_last: bool,
    sibling_count: usize,
    boundary_count: usize,
    size: Size,
}

impl<'a> Pagination<'a> {
    pub fn new(page: &'a mut usize, total_pages: usize) -> Self {
        Self {
            page,
            total_pages,
            compact: false,
            show_first_last: false,
            sibling_count: 1,
            boundary_count: 1,
            size: Size::Sm,
        }
    }

    pub fn compact(mut self, v: bool) -> Self { self.compact = v; self }
    pub fn show_first_last(mut self, v: bool) -> Self { self.show_first_last = v; self }
    pub fn sibling_count(mut self, n: usize) -> Self { self.sibling_count = n; self }
    pub fn boundary_count(mut self, n: usize) -> Self { self.boundary_count = n; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let Pagination { page, total_pages, compact, show_first_last, sibling_count, boundary_count, size } = self;

        let total = total_pages.max(1);
        let cur = (*page).min(total - 1);

        let resp = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = st::gap_2xs();

            if show_first_last {
                let r = Button::new("\u{00AB}")
                    .variant(Variant::Ghost).size(size).disabled(cur == 0).show(ui, theme);
                if r.clicked() { *page = 0; }
            }

            // Prev
            let r = Button::new("\u{2039}")
                .variant(Variant::Ghost).size(size).disabled(cur == 0).show(ui, theme);
            if r.clicked() && cur > 0 { *page = cur - 1; }

            if compact {
                let _ = Label::new(format!("Page {} of {}", cur + 1, total))
                    .size(size).muted().show(ui, theme);
            } else {
                let pages = compute_pages(cur, total, sibling_count, boundary_count);
                for item in pages {
                    match item {
                        PageItem::Num(p) => {
                            let label = format!("{}", p + 1);
                            let variant = if p == cur { Variant::Primary } else { Variant::Ghost };
                            let r = Button::new(label.as_str())
                                .variant(variant).size(size).show(ui, theme);
                            if r.clicked() { *page = p; }
                        }
                        PageItem::Ellipsis => {
                            let _ = Label::new("\u{2026}").size(size).muted().show(ui, theme);
                        }
                    }
                }
            }

            // Next
            let r = Button::new("\u{203A}")
                .variant(Variant::Ghost).size(size).disabled(cur + 1 >= total).show(ui, theme);
            if r.clicked() && cur + 1 < total { *page = cur + 1; }

            if show_first_last {
                let r = Button::new("\u{00BB}")
                    .variant(Variant::Ghost).size(size).disabled(cur + 1 >= total).show(ui, theme);
                if r.clicked() { *page = total - 1; }
            }
        });

        resp.response
    }
}

#[derive(Clone, Copy)]
enum PageItem { Num(usize), Ellipsis }

/// Compute the page-button list with ellipsis collapsing.
/// `current` is 0-indexed; output is also 0-indexed page numbers.
fn compute_pages(current: usize, total: usize, siblings: usize, boundary: usize) -> Vec<PageItem> {
    let total = total.max(1);
    // If everything fits with no collapsing needed, just emit all pages.
    // Threshold: 2*boundary + 2*siblings + 1 (current) + 2 (ellipses) = always show all if smaller.
    let threshold = 2 * boundary + 2 * siblings + 3;
    if total <= threshold {
        return (0..total).map(PageItem::Num).collect();
    }

    let mut out = Vec::new();
    let last = total - 1;

    // Start range
    let start_end = boundary.min(total).saturating_sub(1);
    for p in 0..=start_end {
        out.push(PageItem::Num(p));
    }

    // Middle range around current
    let mid_start = current.saturating_sub(siblings).max(boundary);
    let mid_end = (current + siblings).min(last.saturating_sub(boundary));

    if mid_start > start_end + 1 {
        out.push(PageItem::Ellipsis);
    } else {
        // Fill the gap if no ellipsis needed.
        for p in (start_end + 1)..mid_start {
            out.push(PageItem::Num(p));
        }
    }

    if mid_end >= mid_start {
        for p in mid_start..=mid_end {
            out.push(PageItem::Num(p));
        }
    }

    // End range
    let end_start = last.saturating_sub(boundary.saturating_sub(1)).max(mid_end + 1);

    if end_start > mid_end + 1 {
        out.push(PageItem::Ellipsis);
    } else {
        for p in (mid_end + 1)..end_start {
            out.push(PageItem::Num(p));
        }
    }

    for p in end_start..total {
        out.push(PageItem::Num(p));
    }

    out
}
