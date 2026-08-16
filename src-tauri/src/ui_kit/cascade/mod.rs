//! The declarative cascading layer — an ORGANIZING system over egui.
//!
//! # What this is, and what it is not
//!
//! It is a way to *write* and *resolve* UI: a real parent→child cascade
//! ([`context`]) and a declarative element tree ([`element`]) that composes
//! into the layout engine and paints through egui.
//!
//! It is **not** a renderer, a virtual DOM, or a reconciler. There is no
//! diffing, no keys, no component lifecycle, no retained node graph. The tree
//! is built and consumed within a single frame, exactly as egui already works —
//! this compiles down to the same `ui.painter()` calls the app makes today.
//!
//! The distinction matters because "like Dioxus" could mean either thing.
//! Dioxus's defining machinery is a VDOM over a webview; what we want from it
//! is the *authoring model* — declaration instead of sequence, and a cascade
//! instead of a global lookup — on top of an immediate-mode painter that
//! already suits this app.
//!
//! # The two halves
//!
//! * [`context`] — inheritance. CSS's inheritable properties (`color`,
//!   `font-*`, `letter-spacing`, `text-align`) flow down; box properties do
//!   not. Absent means "the widget's own default", so unscoped code is
//!   unchanged.
//! * [`element`] — declaration. A tree of nodes with layout intent, solved by
//!   the Taffy-backed [`crate::ui_kit::layout`] and painted in one pass.
//!
//! They are separable on purpose: a surface can adopt the cascade without the
//! element tree, or vice versa, and neither requires rewriting a widget first.

pub mod context;
pub mod element;

pub use context::{resolved, scope, Inherited};
