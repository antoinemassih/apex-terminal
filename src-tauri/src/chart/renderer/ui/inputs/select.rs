//! Builder + impl Widget primitives — select / dropdown family.
//!
//! Wave 5 introduces typed selection primitives that wrap egui::ComboBox /
//! popup machinery with the project theme + style tokens. These are NEW
//! additions; existing call-sites are not migrated yet.
//!
//! All builders are generic over the value type. Dropdown / Combobox /
//! RadioGroup / SegmentedControl require `T: PartialEq + Copy`, while
//! MultiSelect additionally needs `T: Eq + std::hash::Hash` for the
//! HashSet variant.
//!
//! See ui/widgets/mod.rs for the rationale.



use egui::{Color32, Response, RichText, Stroke, Ui};
use super::super::style::*;

#[inline(always)]
fn ambient(ctx: &egui::Context) -> super::super::super::gpu::Theme {
    crate::chart_renderer::theme_impl::active_theme(ctx)
}

// ─── Dropdown ─────────────────────────────────────────────────────────────────

/// Single-value dropdown selector. Click opens a popup list of `(T, label)`
/// pairs. Returns `true` from `.show(...)` if the value was changed.
///
/// ```ignore
/// let mut chart_kind = ChartKind::Candle;
/// let opts = [(ChartKind::Candle, "Candle"), (ChartKind::Line, "Line")];
/// if Dropdown::new().options(&opts).theme(t).show(ui, &mut chart_kind) {
///     // changed
/// }
/// ```
#[must_use = "Dropdown must be rendered via `.show(ui, &mut value)`"]
pub struct Dropdown<'a, T: PartialEq + Copy> {
    label: Option<&'a str>,
    options: &'a [(T, &'a str)],
    width: Option<f32>,
    accent: Option<Color32>,
    dim: Option<Color32>,
}

impl<'a, T: PartialEq + Copy> Dropdown<'a, T> {
    /// No id salt.
    ///
    /// This took one and stored it without ever reading it. `show` delegates to
    /// `ui_kit::widgets::Select`, which derives its identity from `ui` the way
    /// egui does for every other widget, so the salt became vestigial when this
    /// was migrated to delegate — and callers went on passing distinct strings
    /// ("heat_idx", "spread_strategy_combo") in the reasonable belief that
    /// identity depended on them. A parameter that looks load-bearing and is
    /// ignored is the same defect class as a button that looks pressable and
    /// does nothing.
    pub fn new() -> Self {
        Self {
            label: None,
            options: &[],
            width: None,
            accent: None,
            dim: None,
        }
    }
    pub fn label(mut self, l: &'a str) -> Self { self.label = Some(l); self }
    pub fn options(mut self, opts: &'a [(T, &'a str)]) -> Self { self.options = opts; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }

    pub fn show(self, ui: &mut Ui, current: &mut T) -> bool {
        let dim = self.dim.unwrap_or(ambient(ui.ctx()).dim);

        if let Some(l) = self.label {
            ui.label(RichText::new(l).monospace().size(font_sm()).color(dim));
        }

        // Build label list + locate current index.
        let labels: Vec<&str> = self.options.iter().map(|(_, s)| *s).collect();
        let mut idx: usize = self.options.iter().position(|(v, _)| v == current).unwrap_or(0);
        let prev = idx;
        let ct = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
        let mut sel = crate::ui_kit::widgets::Select::new(&mut idx, &labels);
        if let Some(w) = self.width { sel = sel.min_width(w); }
        let resp = sel.show(ui, &ct);
        let changed = resp.changed && idx != prev && idx < self.options.len();
        if changed { *current = self.options[idx].0; }
        changed
    }
}

// ─── Autocomplete ─────────────────────────────────────────────────────────────

/// Free-text input with a filtered suggestion popup. Returns `Some(picked)`
/// on the frame the user accepts a suggestion (click); otherwise `None`.
/// The buffer is mutated as the user types.
///
/// ```ignore
/// let mut buf = String::new();
/// if let Some(picked) = Autocomplete::new("ac_symbol", &mut buf)
///     .suggestions(&["AAPL", "MSFT", "SPY"]).theme(t).show(ui)
/// {
///     // user accepted `picked`
/// }
/// ```
#[must_use = "Autocomplete must be rendered via `.show(ui)`"]
pub struct Autocomplete<'a, 'b> {
    id_salt: &'a str,
    buffer: &'b mut String,
    suggestions: &'a [&'a str],
    placeholder: &'a str,
    width: Option<f32>,
    max_visible: usize,
    accent: Option<Color32>,
    dim: Option<Color32>,
}

impl<'a, 'b> Autocomplete<'a, 'b> {
    pub fn new(id_salt: &'a str, buffer: &'b mut String) -> Self {
        Self {
            id_salt,
            buffer,
            suggestions: &[],
            placeholder: "",
            width: None,
            max_visible: 8,
            accent: None,
            dim: None,
        }
    }
    pub fn suggestions(mut self, s: &'a [&'a str]) -> Self { self.suggestions = s; self }
    pub fn placeholder(mut self, p: &'a str) -> Self { self.placeholder = p; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn max_visible(mut self, n: usize) -> Self { self.max_visible = n; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Option<String> {
        let accent = self.accent.unwrap_or(ambient(ui.ctx()).accent);
        let dim = self.dim.unwrap_or(ambient(ui.ctx()).dim);

        let edit_id = egui::Id::new(("autocomplete_edit", self.id_salt));
        let popup_id = egui::Id::new(("autocomplete_popup", self.id_salt));
        let mut picked: Option<String> = None;

        let avail = self.width.unwrap_or_else(|| ui.available_width());
        let edit = egui::TextEdit::singleline(self.buffer)
            .id(edit_id)
            .hint_text(RichText::new(self.placeholder).color(color_alpha(dim, alpha_muted())))
            .font(egui::FontSelection::FontId(crate::ui_kit::style::mono_sm()))
            .desired_width(avail);
        let resp = ui.add(edit);

        let has_focus = resp.has_focus() || resp.gained_focus();
        if has_focus && !self.buffer.is_empty() {
            ui.memory_mut(|m| m.open_popup(popup_id));
        }
        if resp.lost_focus() {
            // close on focus lost (avoid clipping clicks below by deferring? egui handles this)
        }

        let needle = self.buffer.trim().to_lowercase();
        let filtered: Vec<&str> = if needle.is_empty() {
            Vec::new()
        } else {
            self.suggestions
                .iter()
                .copied()
                .filter(|s| s.to_lowercase().contains(&needle))
                .take(self.max_visible)
                .collect()
        };

        if !filtered.is_empty() {
            egui::popup::popup_below_widget(
                ui,
                popup_id,
                &resp,
                egui::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(avail);
                    for s in filtered.iter() {
                        let r = ui.selectable_label(
                            false,
                            RichText::new(*s).monospace().size(font_sm()).color(accent),
                        );
                        if r.clicked() {
                            *self.buffer = s.to_string();
                            picked = Some(s.to_string());
                            ui.memory_mut(|m| m.close_popup());
                        }
                    }
                },
            );
        }

        picked
    }
}

// ─── SegmentedControl ─────────────────────────────────────────────────────────

/// Inline segmented button group — like a horizontal row of pills where
/// exactly one value is active. Single-widget alternative to manually
/// composing `PillButton`s. Returns `true` if the value changed.
///
/// ```ignore
/// let opts = [(Side::Buy, "BUY"), (Side::Sell, "SELL")];
/// SegmentedControl::new().options(&opts).theme(t).show(ui, &mut side);
/// ```
#[must_use = "SegmentedControl must be rendered via `.show(ui, &mut value)`"]
pub struct SegmentedControl<'a, T: PartialEq + Copy> {
    options: &'a [(T, &'a str)],
    accent: Option<Color32>,
    dim: Option<Color32>,
    /// When `true`, segments share edges as connected pills:
    /// first=rounded-left, last=rounded-right, middle=square.
    connected_pills: bool,
    /// When `true`, use tighter padding (suitable for 14px-height pill rows).
    compact: bool,
    /// Optional fixed height override.
    height: Option<f32>,
}

impl<'a, T: PartialEq + Copy> SegmentedControl<'a, T> {
    pub fn new() -> Self {
        Self {
            options: &[],
            accent: None,
            dim: None,
            connected_pills: false,
            compact: false,
            height: None,
        }
    }
    pub fn options(mut self, opts: &'a [(T, &'a str)]) -> Self { self.options = opts; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }
    /// Paint segments as connected pills with shared inner edges.
    /// First segment gets rounded left corners, last gets rounded right corners,
    /// middle segments are square. A single segment gets all corners rounded.
    pub fn connected_pills(mut self, v: bool) -> Self { self.connected_pills = v; self }
    /// Tighter padding for compact pill rows (e.g. indicator_editor 14px rows).
    pub fn compact(mut self, v: bool) -> Self { self.compact = v; self }
    /// Override the minimum height of each segment button.
    pub fn height(mut self, h: f32) -> Self { self.height = Some(h); self }

    pub fn show(self, ui: &mut Ui, current: &mut T) -> bool {
        let accent = self.accent.unwrap_or(ambient(ui.ctx()).accent);
        let dim = self.dim.unwrap_or(ambient(ui.ctx()).dim);
        let mut changed = false;

        let prev_item_spacing = ui.spacing().item_spacing.x;
        let prev_pad = ui.spacing().button_padding;

        if self.connected_pills {
            // Zero gap between segments so they touch and share borders.
            ui.spacing_mut().item_spacing.x = 0.0;
            let pad_y = if self.compact { 1.0 } else { gap_xs() };
            ui.spacing_mut().button_padding = egui::vec2(gap_sm(), pad_y);
        } else {
            ui.spacing_mut().item_spacing.x = gap_xs();
            let pad_y = if self.compact { 1.0 } else { gap_xs() };
            ui.spacing_mut().button_padding = egui::vec2(gap_md(), pad_y);
        }

        let min_h = self.height.unwrap_or(if self.compact { 14.0 } else { 20.0 });
        let n = self.options.len();

        let st = super::super::style::current();
        ui.horizontal(|ui| {
            for (i, (val, label)) in self.options.iter().enumerate() {
                let active = val == current;
                // Resolve active/idle colors from style overrides.
                // Derive active/idle colors from the invert-active discriminant (§3.2).
                let theme = ambient(ui.ctx());
                let active_fill = if st.invert_active_fill { theme.text } else { color_alpha(accent, alpha_tint()) };
                let active_fg   = if st.invert_active_fill { theme.bg } else { accent };
                let idle_fg     = dim; // segmented_idle_text fallback was dim
                let fg = if active { active_fg } else { idle_fg };
                let idle_border_col = theme.toolbar_border; // idle_outline_color fallback was toolbar_border
                let (bg, border) = if active {
                    let fill = if st.invert_active_fill { active_fill } else { color_alpha(accent, alpha_tint()) };
                    (fill, color_alpha(accent, alpha_dim()))
                } else {
                    let idle_bg = Color32::TRANSPARENT; // segmented_idle_fill fallback was TRANSPARENT
                    (idle_bg, idle_border_col)
                };

                let corner_r: egui::CornerRadius = if self.connected_pills {
                    let r = crate::ui_kit::style::radius_sm().round() as u8;
                    let is_first = i == 0;
                    let is_last = i == n.saturating_sub(1);
                    match (is_first, is_last) {
                        (true, true)  => egui::CornerRadius::same(r),
                        (true, false) => egui::CornerRadius { nw: r, sw: r, ne: 0, se: 0 },
                        (false, true) => egui::CornerRadius { nw: 0, sw: 0, ne: r, se: r },
                        (false, false) => egui::CornerRadius::ZERO,
                    }
                } else {
                    egui::CornerRadius::same(radius_pill() as u8)
                };

                // Intentional low-level egui::Button — pill segment with
                // dynamic corner-radius depending on first/middle/last
                // position. ui_kit::Button's Variant system doesn't expose
                // per-corner radius; would need a new variant or escape
                // hatch. Kept as-is; this is a legitimate low-level use.
                let resp = ui.add(
                    egui::Button::new(
                        RichText::new(*label).monospace().size(font_sm()).strong().color(fg),
                    )
                    .fill(bg)
                    .stroke(Stroke::new(stroke_thin(), border))
                    .corner_radius(corner_r)
                    .min_size(egui::vec2(0.0, min_h)),
                );
                if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() && !active {
                    *current = *val;
                    changed = true;
                }
            }
        });

        ui.spacing_mut().button_padding = prev_pad;
        ui.spacing_mut().item_spacing.x = prev_item_spacing;
        changed
    }
}

impl<'a, T: PartialEq + Copy> Default for SegmentedControl<'a, T> {
    fn default() -> Self { Self::new() }
}

// ─── DropdownOwned ────────────────────────────────────────────────────────────

/// String-key / dynamic-label dropdown for `T: Clone + PartialEq`.
/// Unlike `Dropdown<T>`, this type owns its option list (`Vec<(T, String)>`)
/// so it works with runtime-computed labels and non-`Copy` keys such as
/// `String` or enum variants with payloads.
///
/// Returns `true` from `.show(...)` if the selected value changed.
#[must_use = "DropdownOwned must be rendered via `.show(ui, &mut value)` or `.show_resp(...)`"]
pub struct DropdownOwned<'a, T: Clone + PartialEq> {
    label: Option<&'a str>,
    options: Vec<(T, String)>,
    width: Option<f32>,
    font_size: Option<f32>,
    selected_text: Option<String>,
    item_context_menu: Option<Box<dyn FnMut(&T, &mut Ui) + 'a>>,
    accent: Option<Color32>,
    dim: Option<Color32>,
}

impl<'a, T: Clone + PartialEq> DropdownOwned<'a, T> {
    /// No id salt — see `Dropdown::new` for why it was removed.
    pub fn new() -> Self {
        Self {
            label: None,
            options: Vec::new(),
            width: None,
            font_size: None,
            selected_text: None,
            item_context_menu: None,
            accent: None,
            dim: None,
        }
    }
    pub fn label(mut self, l: &'a str) -> Self { self.label = Some(l); self }
    pub fn options(mut self, opts: Vec<(T, String)>) -> Self { self.options = opts; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn font_size(mut self, s: f32) -> Self { self.font_size = Some(s); self }
    pub fn selected_text(mut self, s: impl Into<String>) -> Self { self.selected_text = Some(s.into()); self }
    pub fn item_context_menu(mut self, f: impl FnMut(&T, &mut Ui) + 'a) -> Self {
        self.item_context_menu = Some(Box::new(f)); self
    }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }

    /// Show the dropdown. Returns `true` if the value changed.
    pub fn show(self, ui: &mut Ui, current: &mut T) -> bool {
        self.show_resp(ui, current).0
    }

    /// Show the dropdown. Returns `(changed, combo_response)`.
    pub fn show_resp(mut self, ui: &mut Ui, current: &mut T) -> (bool, Response) {
        let dim = self.dim.unwrap_or(ambient(ui.ctx()).dim);

        if let Some(l) = self.label {
            ui.label(RichText::new(l).monospace().size(font_sm()).color(dim));
        }

        let labels: Vec<String> = self.options.iter().map(|(_, s)| s.clone()).collect();
        let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let mut idx: usize = self.options.iter().position(|(v, _)| v == current).unwrap_or(0);
        let prev = idx;
        let ct = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
        let mut sel = crate::ui_kit::widgets::Select::new(&mut idx, &label_refs);
        if let Some(w) = self.width { sel = sel.min_width(w); }
        // Wire per-item context menu against the owned option values.
        // Clone option values into an owned Vec so the closure can be `move`
        // without conflicting with the post-show borrow of self.options.
        if let Some(ctx_fn) = self.item_context_menu.take() {
            let menu_vals: Vec<T> = self.options.iter().map(|(v, _)| v.clone()).collect();
            // `DropdownOwned`'s menu closure is `FnMut`, but `Select` stores a
            // `Fn`; a `RefCell` lets the outer `Fn` closure call the inner
            // `FnMut` (borrowed mutably only for the synchronous invocation).
            let ctx_fn = std::cell::RefCell::new(ctx_fn);
            sel = sel.item_context_menu(move |i, ui| {
                if let Some(v) = menu_vals.get(i) { (ctx_fn.borrow_mut())(v, ui); }
            });
        }
        let resp = sel.show(ui, &ct);
        let changed = resp.changed && idx != prev && idx < self.options.len();
        if changed { *current = self.options[idx].0.clone(); }
        (changed, resp.response)
    }
}

