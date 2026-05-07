//! Modal has been re-homed to `ui_kit::widgets::modal`.
//! This file is a thin shim so existing call sites importing
//! `widgets::chrome::*` (or `chrome::modal::*`) keep compiling.
//!
//! Migration plan: callers should switch to
//! `crate::ui_kit::widgets::Modal`. After that sweep, this file goes away.

#[deprecated(note = "use crate::ui_kit::widgets::Modal")]
pub use crate::ui_kit::widgets::modal::Modal;
#[deprecated(note = "use crate::ui_kit::widgets::modal::Anchor")]
pub use crate::ui_kit::widgets::modal::Anchor;
#[deprecated(note = "use crate::ui_kit::widgets::modal::HeaderStyle")]
pub use crate::ui_kit::widgets::modal::HeaderStyle;
#[deprecated(note = "use crate::ui_kit::widgets::modal::FrameKind")]
pub use crate::ui_kit::widgets::modal::FrameKind;
#[deprecated(note = "use crate::ui_kit::widgets::modal::ModalResponse")]
pub use crate::ui_kit::widgets::modal::ModalResponse;
