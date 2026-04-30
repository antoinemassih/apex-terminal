//! Canonical chrome components — style-aware (Relay/Meridien) building blocks.
//!
//! These encapsulate paint patterns repeated across panels/dialogs. All radii,
//! strokes, and treatments route through `super::style` so a single style flip
//! propagates everywhere. Colors are passed in by callers (no `Theme` coupling).
//!
//! Split into submodules by concern (labels, pills, frames, headers, hairlines,
//! metrics). Everything is re-exported here so external callers continue to use
//! `components::foo` without source changes.

#![allow(dead_code)]

pub mod labels;
pub mod pills;
pub mod frames;
pub mod headers;
pub mod hairlines;
pub mod metrics;

pub use labels::*;
pub use pills::*;
pub use frames::*;
pub use headers::*;
pub use hairlines::*;
pub use metrics::*;
