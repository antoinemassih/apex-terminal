//! Pane templates ("Day Trader", "Scalper", etc.). First-class entities.
//! Charts may reference one via `ChartState::template_id`, but charts remain
//! valid if the template is later deleted (`ON DELETE SET NULL`).

use arcstr::ArcStr;
use serde::{Deserialize, Serialize};

use super::extension_bag::ExtensionBag;
use super::indicators::IndicatorRef;

pub type TemplateId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneSpec {
    pub id: ArcStr,
    /// Height as a percentage of the chart area (0..=100).
    pub height_pct: u8,
    /// Indicators that belong in this pane by default when the template is applied.
    pub indicators: Vec<ArcStr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneTemplate {
    pub id: TemplateId,
    pub name: ArcStr,
    pub version: u32,
    pub panes: Vec<PaneSpec>,
    /// Concrete indicator refs (with params) that get installed into the
    /// chart's `indicators` list when the template is applied.
    pub default_indicators: Vec<IndicatorRef>,

    #[serde(default, skip_serializing_if = "ExtensionBag::is_empty")]
    pub extras: ExtensionBag,
}
