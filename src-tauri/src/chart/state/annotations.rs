//! Annotations — first-class commentary, separate from drawings.

use arcstr::ArcStr;
use serde::{Deserialize, Serialize};
use slotmap::new_key_type;
use smallvec::SmallVec;

use super::drawings::Point;
use super::extension_bag::ExtensionBag;

new_key_type! {
    pub struct AnnotationId;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub anchor: Point,
    pub title: ArcStr,
    /// Restricted Markdown — no HTML, no scripts, no remote images.
    /// Sanitized on render.
    pub body_md: ArcStr,
    /// 0xRRGGBBAA packed.
    pub color: u32,
    /// Asset references (e.g., `assets/<uuid>.png`) — empty inline by default.
    #[serde(default, skip_serializing_if = "SmallVec::is_empty")]
    pub asset_refs: SmallVec<[ArcStr; 2]>,
    #[serde(default, skip_serializing_if = "ExtensionBag::is_empty")]
    pub extras: ExtensionBag,
}
