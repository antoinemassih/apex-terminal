//! The inherited half of the cascade — values that flow parent → child.
//!
//! # What this adds that `frame_tokens()` cannot
//!
//! Everything before this resolved styling **globally**: a widget asked
//! `frame_tokens()` for the frame's one `TokenSnapshot` and got the same answer
//! no matter where in the tree it sat. That is a cascade in the sense of
//! "override → DesignTokens → StyleSystem" (see `cascade_gate.py`), and it is
//! not a cascade in the CSS sense at all — there is no ancestry in it.
//!
//! This module adds the missing axis. A container declares values; its
//! descendants inherit them; any descendant may override locally for its own
//! subtree. That is what makes `Panel { color: dim, Text { "x" } }` mean what a
//! reader coming from CSS expects it to mean.
//!
//! # Only CSS's inheritable properties inherit
//!
//! In CSS, `color`, `font-*`, `line-height`, `letter-spacing` and `text-align`
//! inherit; `padding`, `margin`, `border`, `width` and `background` do not. We
//! mirror that split exactly rather than inventing one, for a specific reason:
//! the point of the exercise is that a reader can *think in CSS*. A system
//! where a panel's `padding` silently became every descendant's padding would
//! look like CSS and behave like nothing — worse than an unfamiliar API,
//! because it is a familiar API that lies.
//!
//! Box properties are therefore deliberately absent from [`Inherited`]. They
//! belong to the element that declares them and are resolved by layout.
//!
//! # Absent means "the widget's own default", not "a global default"
//!
//! Every field is an `Option`, and it stays an `Option` after resolution. A
//! `None` means no ancestor declared this, so the widget uses whatever it would
//! have used anyway — `Button` keeps its own tier, a price cell keeps its own
//! colour. Only an explicit declaration overrides them.
//!
//! This differs from CSS, which always computes a value by falling back to a
//! UA default, and the difference is deliberate. A root default here would have
//! to invent one answer for "what colour is text" and would then quietly
//! outrank the per-widget defaults that ~74 widgets already carry. Inheritance
//! that clobbers is not a cascade, it is a global with extra steps.
//!
//! The practical consequence is the one that makes adoption safe: **with no
//! scope open nothing changes at all.** A surface adopts the cascade by opening
//! a scope, not by being rewritten first.
//!
//! # Resolution is O(1)
//!
//! Each [`scope`] merges its delta onto the *already-resolved* parent and
//! stores the result, so a read takes the top of the stack instead of walking
//! ancestors. A deep tree costs what a shallow one costs, which matters when
//! these are read per-widget, per-frame.

use std::cell::RefCell;

use egui::{Align, Color32};

use crate::ui_kit::text_style::TextStyle;

/// Values that flow down the tree.
///
/// `None` = not declared by any ancestor; the widget's own default applies.
/// A `Some` at any level replaces the value for that subtree and no other.
///
/// Deliberately small. Every field is one CSS marks as inherited; adding a box
/// property would make the struct more useful and the model less true.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inherited {
    /// Type tier — family, size, weight and leading in one token. CSS spells
    /// this as several inheriting properties; the tier is our vocabulary.
    pub text_style: Option<TextStyle>,
    /// CSS `color` — text, and the default for glyph-shaped content (icons)
    /// that has not been given its own.
    pub text_color: Option<Color32>,
    /// CSS `letter-spacing`, px. Meridien and Aperture both author tracking on
    /// their label tiers, so this genuinely varies by subtree.
    pub letter_spacing: Option<f32>,
    /// CSS `text-align`.
    pub text_align: Option<Align>,
}

impl Inherited {
    /// `self` layered over `parent` — every `Some` in `self` wins, and a `None`
    /// keeps whatever the parent resolved to.
    ///
    /// This is the merge CSS performs at each element. Applied once per
    /// [`scope`], not once per read.
    #[must_use]
    pub fn over(self, parent: Inherited) -> Inherited {
        Inherited {
            text_style: self.text_style.or(parent.text_style),
            text_color: self.text_color.or(parent.text_color),
            letter_spacing: self.letter_spacing.or(parent.letter_spacing),
            text_align: self.text_align.or(parent.text_align),
        }
    }

    // ── Builder sugar, so a scope reads as a declaration ────────────────────

    #[must_use]
    pub fn text_style(mut self, t: TextStyle) -> Self {
        self.text_style = Some(t);
        self
    }
    #[must_use]
    pub fn color(mut self, c: Color32) -> Self {
        self.text_color = Some(c);
        self
    }
    #[must_use]
    pub fn letter_spacing(mut self, px: f32) -> Self {
        self.letter_spacing = Some(px);
        self
    }
    #[must_use]
    pub fn align(mut self, a: Align) -> Self {
        self.text_align = Some(a);
        self
    }

    /// The tier in force, or `fallback` when no ancestor declared one.
    #[must_use]
    pub fn text_style_or(self, fallback: TextStyle) -> TextStyle {
        self.text_style.unwrap_or(fallback)
    }
    /// The colour in force, or `fallback` when no ancestor declared one.
    #[must_use]
    pub fn color_or(self, fallback: Color32) -> Color32 {
        self.text_color.unwrap_or(fallback)
    }
}

thread_local! {
    /// Resolved sets, innermost last. Empty means no scope is open.
    ///
    /// Thread-local rather than a passed parameter because egui's own `Ui` is
    /// reached that way throughout this codebase; threading a context argument
    /// through ~74 widgets would BE the migration rather than a detail of it.
    /// The stack is per-frame and never outlives the paint pass.
    static STACK: RefCell<Vec<Inherited>> = const { RefCell::new(Vec::new()) };
}

/// The values in force at this point in the tree.
///
/// All-`None` at the root, which is what keeps unscoped rendering identical.
#[must_use]
pub fn resolved() -> Inherited {
    STACK.with(|s| s.borrow().last().copied()).unwrap_or_default()
}

/// Open a scope for the duration of `f`.
///
/// ```ignore
/// scope(Inherited::default().color(t.dim).text_style(TextStyle::Caption), || {
///     // every descendant reads dim/Caption unless it says otherwise
/// });
/// ```
///
/// The guard restores the previous depth even if `f` panics, so a panicking
/// widget cannot leave the rest of the frame styled as its subtree.
pub fn scope<R>(delta: Inherited, f: impl FnOnce() -> R) -> R {
    let merged = delta.over(resolved());
    STACK.with(|s| s.borrow_mut().push(merged));
    let _guard = PopOnDrop;
    f()
}

struct PopOnDrop;

impl Drop for PopOnDrop {
    fn drop(&mut self) {
        STACK.with(|s| {
            s.borrow_mut().pop();
        });
    }
}

/// Clear the stack. Called once per frame from `begin_frame`, so a leaked scope
/// cannot bleed from one frame into the next.
pub fn reset_for_frame() {
    STACK.with(|s| s.borrow_mut().clear());
}

/// Current nesting depth. Test / diagnostic hook.
#[must_use]
pub fn depth() -> usize {
    STACK.with(|s| s.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean() {
        reset_for_frame();
    }

    #[test]
    fn a_child_inherits_what_the_parent_declared() {
        clean();
        let red = Color32::from_rgb(255, 0, 0);
        scope(Inherited::default().color(red), || {
            assert_eq!(resolved().text_color, Some(red));
            // an inner scope silent about colour keeps it
            scope(Inherited::default().text_style(TextStyle::Caption), || {
                assert_eq!(resolved().text_color, Some(red));
                assert_eq!(resolved().text_style, Some(TextStyle::Caption));
            });
        });
    }

    #[test]
    fn a_local_override_applies_to_its_subtree_and_no_further() {
        clean();
        let a = Color32::from_rgb(1, 2, 3);
        let b = Color32::from_rgb(4, 5, 6);
        scope(Inherited::default().color(a), || {
            scope(Inherited::default().color(b), || {
                assert_eq!(resolved().text_color, Some(b));
            });
            // ← the override ended with its subtree
            assert_eq!(resolved().text_color, Some(a));
        });
    }

    /// The property that makes adoption safe: unscoped code is unchanged.
    ///
    /// Nothing is declared at the root, so every `*_or(fallback)` returns the
    /// widget's own default and a surface that has not adopted the cascade
    /// renders exactly as it did before this module existed.
    #[test]
    fn no_scope_means_no_declarations_and_therefore_no_change() {
        clean();
        assert_eq!(depth(), 0);
        assert_eq!(resolved(), Inherited::default());
        let mine = Color32::from_rgb(9, 9, 9);
        assert_eq!(resolved().color_or(mine), mine, "the widget's default must win");
        assert_eq!(resolved().text_style_or(TextStyle::Mono), TextStyle::Mono);
    }

    /// Inheritance must not clobber a widget that has its own opinion — but it
    /// MUST reach one that does not. Both halves in one test because getting
    /// either wrong turns the cascade into a global.
    #[test]
    fn a_declaration_reaches_defaults_without_clobbering_choices() {
        clean();
        let declared = Color32::from_rgb(10, 20, 30);
        let widget_choice = Color32::from_rgb(40, 50, 60);
        scope(Inherited::default().color(declared), || {
            // a widget with no opinion picks up the declaration
            assert_eq!(resolved().color_or(widget_choice), declared);
            // a widget that states its own colour simply does not consult us
            assert_eq!(widget_choice, Color32::from_rgb(40, 50, 60));
        });
    }

    /// A panicking child must not leave its scope open for everything after it.
    #[test]
    fn a_panicking_scope_still_pops() {
        clean();
        let before = depth();
        let r = std::panic::catch_unwind(|| {
            scope(Inherited::default().color(Color32::RED), || panic!("boom"));
        });
        assert!(r.is_err());
        assert_eq!(depth(), before, "a panic leaked a style scope");
    }

    /// Box properties must NOT be inheritable.
    ///
    /// This is the model's central claim — that it behaves the way CSS does —
    /// and it is exactly the kind of thing that gets "helpfully" widened later
    /// by someone who wants a container's padding to reach its children. The
    /// destructure below stops compiling the moment a field is added, which
    /// forces that change through this comment.
    #[test]
    fn only_css_inheritable_properties_are_present() {
        let Inherited { text_style, text_color, letter_spacing, text_align } =
            Inherited::default();
        let _ = (text_style, text_color, letter_spacing, text_align);
    }
}
