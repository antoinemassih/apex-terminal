//! Debug annotation overlays — coloured rects/labels painted on the live app canvas.
//!
//! Driven from outside via POST /annotations. Rendered each frame in `render_annotations()`.
//! Zero impact on release builds (entire dev_inspector module is cfg(debug_assertions)).

use crate::dev_inspector::SerRect;

/// A coloured rect (filled or border-only) with an optional text label.
/// Rendered as an egui Tooltip-layer overlay above all chart content.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DebugAnnotation {
    /// Stable identifier. POST /annotations merges by id, so re-sending the same id updates in place.
    pub id: String,
    /// Rectangle in screen-space pixels.
    pub rect: SerRect,
    /// Optional label drawn at rect.min_x, rect.min_y.
    #[serde(default)]
    pub label: String,
    /// RGBA colour [r, g, b, a] in 0-255 range.
    #[serde(default = "default_color")]
    pub color: [u8; 4],
    /// If true, only the border is drawn (alpha controls border opacity).
    #[serde(default)]
    pub border_only: bool,
    /// Border stroke width in pixels. Defaults to 1.0.
    pub border_width: Option<f32>,
    /// Optional category tag for filtering/grouping (e.g. "layout", "design", "model").
    pub tag: Option<String>,
}

fn default_color() -> [u8; 4] { [100, 180, 255, 80] }

/// Mutation operation applied to the active annotation list.
#[derive(Debug, Clone)]
pub enum AnnotationOp {
    /// Upsert a list of annotations (merge by id).
    Upsert(Vec<DebugAnnotation>),
    /// Remove annotation with the given id.
    Remove(String),
    /// Remove all annotations, optionally filtered by tag.
    Clear(Option<String>),
}

/// Apply one AnnotationOp to an existing annotation list.
pub fn apply_op(list: &mut Vec<DebugAnnotation>, op: AnnotationOp) {
    match op {
        AnnotationOp::Upsert(incoming) => {
            for ann in incoming {
                if let Some(existing) = list.iter_mut().find(|a| a.id == ann.id) {
                    *existing = ann;
                } else {
                    list.push(ann);
                }
            }
        }
        AnnotationOp::Remove(id) => {
            list.retain(|a| a.id != id);
        }
        AnnotationOp::Clear(tag_filter) => {
            match tag_filter {
                None => list.clear(),
                Some(tag) => list.retain(|a| a.tag.as_deref() != Some(&tag)),
            }
        }
    }
}
