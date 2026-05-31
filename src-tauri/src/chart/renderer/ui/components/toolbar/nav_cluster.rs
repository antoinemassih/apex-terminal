//! `NavCluster` — a named, independently-stylable group of top-nav items.
//!
//! The top navigation is composed of a sequence of clusters, each tagged with a
//! [`NavRole`]. A cluster wraps its content in a frame styled from the active
//! style's `nav_cluster_*` tokens (radius, fill alpha, padding), so a theme can
//! make e.g. the search a pill, the brand a flat block, the account a circle —
//! without the toolbar code hardcoding any of it.
//!
//! This is the building block that turns the old inline `toolbar_btn()` soup
//! into segmented, designable units (the ApertureJune layout reference).

use egui::{Color32, CornerRadius, Margin, Response, Ui};
use super::super::super::style::{current, color_alpha};

type Theme = crate::chart_renderer::gpu::Theme;

/// Identity of a top-nav cluster. Drives which palette colour the cluster
/// background tints toward (when `nav_cluster_fill_alpha > 0`) and lets future
/// per-role overrides hang off a stable key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavRole {
    /// Logo + account-balance block at the far left.
    Brand,
    /// Primary workspace/tab group (Dashboard / Portfolio / …).
    Workspaces,
    /// Secondary app/function group (Trade / Research / …).
    Apps,
    /// The ⌘K search / command field.
    Search,
    /// The trailing account avatar / menu.
    Account,
    /// Second-layer tool group (indicators, drawing tools).
    Tools,
    /// The scrolling symbol/price ticker strip.
    Ticker,
}

impl NavRole {
    /// Palette colour a cluster background tints toward when filled.
    fn fill_base(self, t: &Theme) -> Color32 {
        match self {
            // Search / brand lean on the surface tone; account on accent.
            NavRole::Account => t.accent,
            _ => t.toolbar_border,
        }
    }
}

/// A stylable top-nav cluster. Build with [`NavCluster::new`], render with
/// [`NavCluster::show`].
#[must_use = "NavCluster does nothing until `.show(...)` is called"]
pub struct NavCluster {
    role: NavRole,
    /// Override the cluster fill alpha (else uses the style token).
    fill_alpha: Option<u8>,
    /// Override the cluster corner radius (else uses the style token).
    radius: Option<f32>,
}

impl NavCluster {
    pub fn new(role: NavRole) -> Self {
        Self { role, fill_alpha: None, radius: None }
    }

    /// Force a specific background fill alpha for this cluster (0 = transparent).
    pub fn fill_alpha(mut self, a: u8) -> Self { self.fill_alpha = Some(a); self }

    /// Force a specific corner radius for this cluster's background.
    pub fn radius(mut self, r: f32) -> Self { self.radius = Some(r); self }

    /// Render the cluster: wraps `body` in a horizontally-centred frame styled
    /// from the active `nav_cluster_*` tokens. Returns the frame `Response`.
    pub fn show(self, ui: &mut Ui, t: &Theme, body: impl FnOnce(&mut Ui)) -> Response {
        let st = current();
        let fill_a = self.fill_alpha.unwrap_or(st.nav_cluster_fill_alpha);
        let radius = self.radius.unwrap_or(st.nav_cluster_radius);
        let pad    = st.nav_cluster_padding.max(0.0);

        let mut frame = egui::Frame::NONE
            .inner_margin(Margin::symmetric(pad as i8, 0))
            .corner_radius(CornerRadius::same(radius as u8));
        if fill_a > 0 {
            frame = frame.fill(color_alpha(self.role.fill_base(t), fill_a));
        }

        frame
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| body(ui));
            })
            .response
    }
}
