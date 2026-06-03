//! `Recipe` — variant-driven style resolution (the cva / class-variance-authority
//! layer). A component declares its base style plus named variant groups *once*;
//! call sites pick variant values and get a resolved [`Sx`].
//!
//! **Performance:** a `Recipe` is built once (typically a `LazyLock` static), so
//! the per-frame cost is just walking a handful of small variant tables and
//! merging `Copy` `SxDelta`s — no allocation. Variant lookups use `&'static str`
//! keys; for very hot paths the resolved `Sx` can be cached by the caller.

use super::style::{Sx, SxDelta};

struct VariantGroup {
    name: &'static str,
    default: &'static str,
    options: Vec<(&'static str, SxDelta)>,
}

/// A reusable component style with named variant axes.
pub struct Recipe {
    base: Sx,
    groups: Vec<VariantGroup>,
}

impl Recipe {
    pub fn new(base: Sx) -> Self {
        Recipe { base, groups: Vec::new() }
    }

    /// Declare a variant axis (e.g. `"intent"` with `primary`/`ghost`/`danger`).
    /// `default` is used when the call site omits this axis.
    pub fn variant(
        mut self,
        name: &'static str,
        default: &'static str,
        options: &[(&'static str, SxDelta)],
    ) -> Self {
        self.groups.push(VariantGroup {
            name,
            default,
            options: options.to_vec(),
        });
        self
    }

    /// Resolve the style for the given variant selections. Unspecified axes use
    /// their declared default.
    pub fn resolve(&self, selected: &[(&str, &str)]) -> Sx {
        let mut sx = self.base;
        for g in &self.groups {
            let val = selected
                .iter()
                .find(|(n, _)| *n == g.name)
                .map(|(_, v)| *v)
                .unwrap_or(g.default);
            if let Some((_, delta)) = g.options.iter().find(|(o, _)| *o == val) {
                sx = sx.with(*delta);
            }
        }
        sx
    }
}
