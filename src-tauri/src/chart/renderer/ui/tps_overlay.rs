//! Boss-key overlay: a full-viewport, fully-opaque fake Microsoft Excel
//! "TPS_Report_FY24_Q3.xlsx" spreadsheet that hides the trading UI when
//! someone walks by.  Uses hardcoded Excel-like colours (NOT the app theme)
//! and renders on `egui::Order::Foreground` so it covers everything.

use egui::{Align2, Color32, FontId, Order, Pos2, Rect, Stroke, Vec2};
use super::style::{stroke_thin, stroke_std, stroke_thick};

// ─── Excel palette ─────────────────────────────────────────────────────────
const XL_TITLE_GREEN: Color32 = Color32::from_rgb(33, 115, 70);
const XL_WHITE:       Color32 = Color32::WHITE;
const XL_GRID_BG:     Color32 = Color32::WHITE;
const XL_HEADER_BG:   Color32 = Color32::from_gray(240);
const XL_GRIDLINE:    Color32 = Color32::from_gray(210);
const XL_TEXT:        Color32 = Color32::from_rgb(32, 32, 32);
const XL_DIM:         Color32 = Color32::from_rgb(100, 100, 100);
const XL_TITLEBAR_BG: Color32 = Color32::from_gray(245);
const XL_MENUBAR_BG:  Color32 = Color32::from_gray(248);
const XL_STATUSBAR_BG:Color32 = Color32::from_gray(230);
const XL_SHEET_ACTIVE:Color32 = Color32::WHITE;
const XL_SHEET_INACT: Color32 = Color32::from_gray(218);
const XL_SEL_BORDER:  Color32 = Color32::from_rgb(16, 124, 65);

// ─── Fonts ──────────────────────────────────────────────────────────────────
fn f(size: f32) -> FontId { FontId::proportional(size) }
fn mono(size: f32) -> FontId { FontId::monospace(size) }

/// Render the full-viewport TPS-report overlay.
///
/// Call this inside the egui frame closure after all normal UI has been drawn.
/// The `egui::Area` uses `Order::Foreground` so it sits on top of everything.
/// Returns `true` if the user asked to dismiss the mask this frame
/// (Esc key, or the fake-Excel ✕ close button).
pub fn render_tps_overlay(ctx: &egui::Context) -> bool {
    let screen = ctx.screen_rect();
    // Esc dismisses the mask (checked outside the Area closure).
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));

    let inner = egui::Area::new(egui::Id::new("tps_overlay_area"))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            // Fill the whole screen so no trading UI leaks through.
            ui.set_min_size(screen.size());
            ui.set_clip_rect(screen);

            let painter = ui.painter_at(screen);

            // ── 1. White base fill ──
            painter.rect_filled(screen, 0.0, XL_WHITE);

            // Pointer sink — allocated FIRST so the title-bar close button
            // (interacted below) sits on top of it and stays clickable.
            ui.allocate_rect(screen, egui::Sense::click_and_drag());

            // Set true if the fake-Excel ✕ (close) button is clicked.
            let mut close_clicked = false;

            let mut y = screen.top();

            // ── 2. Title bar ─────────────────────────────────────────────
            let title_h = 28.0;
            let title_rect = Rect::from_min_size(Pos2::new(screen.left(), y), Vec2::new(screen.width(), title_h));
            painter.rect_filled(title_rect, 0.0, XL_TITLEBAR_BG);
            painter.text(
                Pos2::new(screen.left() + crate::ui_kit::style::gap_sm(), y + title_h * 0.5),
                Align2::LEFT_CENTER,
                "Microsoft Excel — TPS_Report_FY24_Q3.xlsx",
                f(11.5),
                XL_TEXT,
            );
            // Window buttons (right-aligned). The ✕ (close) is a real button —
            // clicking it drops the mask, a natural in-character way out.
            for (i, label) in ["─", "□", "✕"].iter().enumerate() {
                let bx = screen.right() - 28.0 * (3 - i) as f32 - 4.0;
                let brect = Rect::from_min_size(Pos2::new(bx, y + 4.0), Vec2::new(26.0, 20.0));
                let is_close = i == 2;
                let hovered = if is_close {
                    let r = ui.interact(brect, egui::Id::new("tps_close_btn"), egui::Sense::click());
                    if r.clicked() { close_clicked = true; }
                    if r.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    r.hovered()
                } else { false };
                // Windows-style: the close button turns red on hover.
                let (bg, fg) = if is_close && hovered {
                    (Color32::from_rgb(232, 17, 35), Color32::WHITE)
                } else {
                    (XL_GRIDLINE, XL_DIM)
                };
                painter.rect_filled(brect, 2.0, bg);
                painter.text(brect.center(), Align2::CENTER_CENTER, *label, f(10.0), fg);
            }
            // Top accent stripe
            painter.line_segment(
                [Pos2::new(screen.left(), y), Pos2::new(screen.right(), y)],
                // Title-bar accent: 3px > stroke_thick (2.0). Kept as literal
                // because no token tier matches; this is a one-shot per-frame
                // boss-key overlay paint, perf cost negligible.
                Stroke::new(3.0, XL_TITLE_GREEN),
            );
            y += title_h;

            // ── 3. Ribbon / Menu bar ──────────────────────────────────────
            let menu_h = 22.0;
            let menu_rect = Rect::from_min_size(Pos2::new(screen.left(), y), Vec2::new(screen.width(), menu_h));
            painter.rect_filled(menu_rect, 0.0, XL_MENUBAR_BG);
            let menu_items = ["File", "Edit", "View", "Insert", "Format", "Tools", "Data", "Window", "Help"];
            let mut mx = screen.left() + crate::ui_kit::style::gap_sm();
            for item in &menu_items {
                let w = item.len() as f32 * 6.5 + 10.0;
                painter.text(Pos2::new(mx, y + menu_h * 0.5), Align2::LEFT_CENTER, *item, f(11.0), XL_TEXT);
                mx += w;
            }
            painter.line_segment(
                [Pos2::new(screen.left(), y + menu_h - 1.0), Pos2::new(screen.right(), y + menu_h - 1.0)],
                Stroke::new(stroke_std(), XL_GRIDLINE),
            );
            y += menu_h;

            // ── 4. Formula bar ────────────────────────────────────────────
            let formula_h = 22.0;
            let formula_rect = Rect::from_min_size(Pos2::new(screen.left(), y), Vec2::new(screen.width(), formula_h));
            painter.rect_filled(formula_rect, 0.0, XL_MENUBAR_BG);
            // Name box
            let name_box = Rect::from_min_size(Pos2::new(screen.left() + crate::ui_kit::style::gap_xs(), y + 3.0), Vec2::new(52.0, 16.0));
            painter.rect_stroke(name_box, 2.0, Stroke::new(stroke_std(), XL_GRIDLINE), egui::StrokeKind::Outside);
            painter.text(name_box.center(), Align2::CENTER_CENTER, "A1", f(10.5), XL_TEXT);
            // fx label
            painter.text(Pos2::new(screen.left() + 62.0, y + formula_h * 0.5), Align2::LEFT_CENTER, "fx", f(11.0), XL_DIM);
            // Formula input area
            let fi_rect = Rect::from_min_size(Pos2::new(screen.left() + 78.0, y + 3.0), Vec2::new(screen.width() - 84.0, 16.0));
            painter.rect_stroke(fi_rect, 0.0, Stroke::new(stroke_std(), XL_GRIDLINE), egui::StrokeKind::Outside);
            painter.text(
                Pos2::new(fi_rect.left() + crate::ui_kit::style::gap_xs(), y + formula_h * 0.5),
                Align2::LEFT_CENTER,
                "TPS REPORT — TEST PROCEDURE SPECIFICATION",
                f(10.5),
                XL_TEXT,
            );
            painter.line_segment(
                [Pos2::new(screen.left(), y + formula_h - 1.0), Pos2::new(screen.right(), y + formula_h - 1.0)],
                Stroke::new(stroke_std(), XL_GRIDLINE),
            );
            y += formula_h;

            // ── 5. Grid ───────────────────────────────────────────────────
            let status_h  = 20.0;
            let sheet_h   = 22.0;
            let grid_top  = y;
            let grid_bot  = screen.bottom() - status_h - sheet_h;
            let grid_h    = (grid_bot - grid_top).max(0.0);

            let row_header_w = 34.0_f32;
            let row_h        = 18.0_f32;
            let n_rows_vis   = ((grid_h / row_h) as usize).min(42);

            // Column widths for A..N
            let col_headers = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N"];
            let col_widths: Vec<f32> = vec![
                160.0, // A
                76.0,  // B
                100.0, // C
                170.0, // D
                76.0,  // E
                74.0,  // F
                70.0,  // G
                72.0,  // H
                180.0, // I
                60.0, 60.0, 60.0, 60.0, 60.0, // J..N
            ];

            // Header row background
            let hdr_rect = Rect::from_min_size(
                Pos2::new(screen.left(), grid_top),
                Vec2::new(screen.width(), row_h),
            );
            painter.rect_filled(hdr_rect, 0.0, XL_HEADER_BG);

            // Row-number column header (corner cell)
            let corner = Rect::from_min_size(Pos2::new(screen.left(), grid_top), Vec2::new(row_header_w, row_h));
            painter.rect_filled(corner, 0.0, XL_HEADER_BG);
            painter.line_segment([corner.right_top(), corner.right_bottom()], Stroke::new(stroke_std(), XL_GRIDLINE));
            painter.line_segment([corner.left_bottom(), corner.right_bottom()], Stroke::new(stroke_std(), XL_GRIDLINE));

            // Column letter headers
            let mut cx = screen.left() + row_header_w;
            for (i, (hdr, &cw)) in col_headers.iter().zip(col_widths.iter()).enumerate() {
                let crect = Rect::from_min_size(Pos2::new(cx, grid_top), Vec2::new(cw, row_h));
                if i == 0 {
                    // A column header — highlighted green (selected)
                    painter.rect_filled(crect, 0.0, XL_SEL_BORDER);
                    painter.text(crect.center(), Align2::CENTER_CENTER, *hdr, f(10.5), Color32::WHITE);
                } else {
                    painter.text(crect.center(), Align2::CENTER_CENTER, *hdr, f(10.5), XL_TEXT);
                }
                painter.line_segment([crect.right_top(), crect.right_bottom()], Stroke::new(stroke_thin(), XL_GRIDLINE));
                cx += cw;
            }
            painter.line_segment(
                [Pos2::new(screen.left(), grid_top + row_h), Pos2::new(screen.right(), grid_top + row_h)],
                Stroke::new(stroke_std(), XL_GRIDLINE),
            );

            // ── Cell content ─────────────────────────────────────────────
            let cell_data: &[&[&str]] = &[
                // Row 1 — big title
                &["TPS REPORT — TEST PROCEDURE SPECIFICATION", "", "", "", "", "", "", "", ""],
                // Row 2 — subtitle
                &["Reporting Period: FY2024 Q3   ·   Prepared by: Accounts Receivable   ·   Ref: memo \"New Cover-Sheet Policy\"", "", "", "", "", "", "", "", ""],
                // Row 3 — blank
                &["", "", "", "", "", "", "", "", ""],
                // Row 4 — column headers
                &["Report ID", "Date", "Department", "Test Procedure", "Cover Sheet", "Status", "Reviewer", "Variance %", "Notes"],
                // Data rows 5-24
                &["TPS-2024-0831", "2024-07-02", "Accounts Rec.", "Invoice Reconciliation v12",   "YES", "Approved", "B. Lumbergh", "0.2%", "All cover sheets present"],
                &["TPS-2024-0832", "2024-07-03", "HR",            "Stapler Inventory Spec.",      "YES", "Approved", "M. Bolton",   "0.0%", "37 TPS-compliant staplers logged"],
                &["TPS-2024-0833", "2024-07-05", "IT",            "Server Room Access Audit",     "NO",  "Pending",  "S. Smythe",   "1.4%", "Missing new cover sheet"],
                &["TPS-2024-0834", "2024-07-08", "Finance",       "Q3 Expense Pre-Approval",      "YES", "Approved", "B. Lumbergh", "0.0%", ""],
                &["TPS-2024-0835", "2024-07-09", "Operations",    "Vendor Compliance Audit",      "NO",  "Review",   "P. Gibbons",  "3.1%", "Cover sheet format outdated"],
                &["TPS-2024-0836", "2024-07-10", "Accounts Rec.", "Intercompany Wire Check",      "YES", "Approved", "B. Lumbergh", "0.1%", ""],
                &["TPS-2024-0837", "2024-07-11", "HR",            "Benefits Enrollment Spec.",    "YES", "Approved", "M. Bolton",   "0.0%", "Completed on time"],
                &["TPS-2024-0838", "2024-07-15", "IT",            "Backup Restore Verification",  "NO",  "Pending",  "S. Smythe",   "0.8%", "Missing new cover sheet"],
                &["TPS-2024-0839", "2024-07-17", "Legal",         "Contract Template Review",     "YES", "Approved", "D. Wallace",  "0.0%", ""],
                &["TPS-2024-0840", "2024-07-18", "Finance",       "Petty Cash Reconciliation",    "NO",  "Review",   "P. Gibbons",  "2.3%", "Cover sheet not initialled"],
                &["TPS-2024-0841", "2024-07-22", "Operations",    "Facilities Inspection Spec.",  "YES", "Approved", "B. Lumbergh", "0.5%", "Minor variance — approved"],
                &["TPS-2024-0842", "2024-07-23", "Accounts Rec.", "AR Aging Report v3",           "YES", "Approved", "B. Lumbergh", "0.0%", ""],
                &["TPS-2024-0843", "2024-07-24", "IT",            "Patch Compliance Audit",       "NO",  "Pending",  "S. Smythe",   "1.1%", "Missing new cover sheet"],
                &["TPS-2024-0844", "2024-07-29", "HR",            "Headcount Reconciliation",     "YES", "Approved", "M. Bolton",   "0.0%", "Cover sheet attached"],
                &["TPS-2024-0845", "2024-07-30", "Finance",       "Month-End Accrual Spec.",      "YES", "Approved", "D. Wallace",  "0.2%", ""],
                &["TPS-2024-0846", "2024-08-01", "Operations",    "Supply Chain Risk Audit",      "NO",  "Review",   "P. Gibbons",  "4.0%", "Cover sheet missing entirely"],
                &["TPS-2024-0847", "2024-08-05", "Legal",         "NDA Renewal Checklist",        "YES", "Approved", "D. Wallace",  "0.0%", ""],
                &["TPS-2024-0848", "2024-08-06", "Accounts Rec.", "Dunning Letter Spec.",         "YES", "Approved", "B. Lumbergh", "0.1%", ""],
                // Blank
                &["", "", "", "", "", "", "", "", ""],
                // Summary
                &["Cover-Sheet Compliance: 87%", "", "", "", "", "", "", "", ""],
            ];

            let data_start_y = grid_top + row_h;

            for (row_idx, row_cells) in cell_data.iter().enumerate() {
                if row_idx + 1 >= n_rows_vis { break; }
                let ry = data_start_y + row_idx as f32 * row_h;
                if ry + row_h > grid_bot { break; }

                // Row background
                let row_rect = Rect::from_min_size(
                    Pos2::new(screen.left(), ry),
                    Vec2::new(screen.width(), row_h),
                );
                painter.rect_filled(row_rect, 0.0, XL_GRID_BG);

                // Row number
                let rn_rect = Rect::from_min_size(Pos2::new(screen.left(), ry), Vec2::new(row_header_w, row_h));
                painter.rect_filled(rn_rect, 0.0, XL_HEADER_BG);
                painter.text(
                    rn_rect.center(),
                    Align2::CENTER_CENTER,
                    &(row_idx + 1).to_string(),
                    f(9.5),
                    XL_DIM,
                );
                painter.line_segment([rn_rect.right_top(), rn_rect.right_bottom()], Stroke::new(stroke_thin(), XL_GRIDLINE));

                // Cells
                let mut cx2 = screen.left() + row_header_w;
                for (col_idx, &cw) in col_widths.iter().enumerate() {
                    let text = row_cells.get(col_idx).copied().unwrap_or("");

                    // Special formatting
                    let (fnt, color, right_align): (FontId, Color32, bool) =
                        if row_idx == 0 && col_idx == 0 {
                            (f(11.5), XL_TITLE_GREEN, false)
                        } else if row_idx == 3 {
                            (f(10.0), XL_DIM, false)
                        } else if col_idx == 7 {
                            (mono(10.0), XL_TEXT, true)
                        } else if col_idx == 4 {
                            let c = if text == "YES" { Color32::from_rgb(30, 130, 60) }
                                    else if text == "NO" { Color32::from_rgb(180, 40, 40) }
                                    else { XL_TEXT };
                            (f(10.5), c, false)
                        } else if col_idx == 5 {
                            let c = match text {
                                "Approved" => Color32::from_rgb(30, 130, 60),
                                "Pending"  => Color32::from_rgb(180, 100, 0),
                                "Review"   => Color32::from_rgb(150, 30, 150),
                                _          => XL_TEXT,
                            };
                            (f(10.5), c, false)
                        } else {
                            (f(10.5), XL_TEXT, false)
                        };

                    if !text.is_empty() {
                        let (tx, anchor) = if right_align {
                            (cx2 + cw - 4.0, Align2::RIGHT_CENTER)
                        } else {
                            (cx2 + 4.0, Align2::LEFT_CENTER)
                        };
                        painter.text(
                            Pos2::new(tx, ry + row_h * 0.5),
                            anchor,
                            text,
                            fnt,
                            color,
                        );
                    }

                    // Right gridline
                    painter.line_segment(
                        [Pos2::new(cx2 + cw, ry), Pos2::new(cx2 + cw, ry + row_h)],
                        Stroke::new(stroke_thin(), XL_GRIDLINE),
                    );
                    cx2 += cw;
                }
                // Bottom row gridline
                painter.line_segment(
                    [Pos2::new(screen.left(), ry + row_h), Pos2::new(screen.right(), ry + row_h)],
                    Stroke::new(stroke_thin(), XL_GRIDLINE),
                );
            }

            // ── Selected cell border (A1 highlight) ──
            let a1_rect = Rect::from_min_size(
                Pos2::new(screen.left() + row_header_w, grid_top + row_h),
                Vec2::new(col_widths[0], row_h),
            );
            painter.rect_stroke(a1_rect, 0.0, Stroke::new(stroke_thick(), XL_SEL_BORDER), egui::StrokeKind::Outside);

            // ── 6. Sheet tabs ─────────────────────────────────────────────
            let tabs_y = grid_bot;
            let tab_h  = sheet_h;
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(screen.left(), tabs_y), Vec2::new(screen.width(), tab_h)),
                0.0, XL_SHEET_INACT,
            );
            let tabs = [("TPS Reports", true), ("Cover Sheets", false), ("Q3 Summary", false), ("Sheet4", false)];
            let mut tx2 = screen.left() + crate::ui_kit::style::gap_xs();
            for &(label, active) in &tabs {
                let tw = label.len() as f32 * 6.5 + 16.0;
                let tab_rect = Rect::from_min_size(Pos2::new(tx2, tabs_y + 2.0), Vec2::new(tw, tab_h - 2.0));
                let bg = if active { XL_SHEET_ACTIVE } else { XL_SHEET_INACT };
                painter.rect_filled(tab_rect, egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, bg);
                if active {
                    painter.line_segment([tab_rect.left_top(), tab_rect.left_bottom()], Stroke::new(stroke_std(), XL_GRIDLINE));
                    painter.line_segment([tab_rect.right_top(), tab_rect.right_bottom()], Stroke::new(stroke_std(), XL_GRIDLINE));
                    painter.line_segment([tab_rect.left_top(), tab_rect.right_top()], Stroke::new(stroke_thick(), XL_TITLE_GREEN));
                }
                painter.text(tab_rect.center(), Align2::CENTER_CENTER, label, f(10.5), XL_TEXT);
                tx2 += tw + 2.0;
            }

            // ── 7. Status bar ─────────────────────────────────────────────
            let stat_y = screen.bottom() - status_h;
            let stat_rect = Rect::from_min_size(Pos2::new(screen.left(), stat_y), Vec2::new(screen.width(), status_h));
            painter.rect_filled(stat_rect, 0.0, XL_STATUSBAR_BG);
            painter.line_segment(
                [Pos2::new(screen.left(), stat_y), Pos2::new(screen.right(), stat_y)],
                Stroke::new(stroke_std(), XL_GRIDLINE),
            );
            // Status items — absolute x offsets from screen left
            let status_items: &[(&str, f32)] = &[
                ("Ready",     6.0),
                ("Average: 0", screen.width() * 0.50),
                ("Count: 20",  screen.width() * 0.62),
                ("Sum: 0",     screen.width() * 0.72),
                ("100%",       screen.width() * 0.87),
            ];
            for &(label, sx) in status_items {
                painter.text(
                    Pos2::new(screen.left() + sx, stat_y + status_h * 0.5),
                    Align2::LEFT_CENTER,
                    label,
                    f(10.0),
                    XL_DIM,
                );
            }

            close_clicked
        })
        .inner;

    esc || inner
}
