//! Tailwind-style color ramps for the `Sx` utility system.
//!
//! Each semantic [`Tone`] (Accent, Bull, …) is expanded into an 11-step shade
//! ramp (50…950) where 500 is the theme's base color, lower shades mix toward
//! white, and higher shades mix toward black — mirroring Tailwind's palette.
//!
//! **Performance:** ramps are derived once per theme and cached in a
//! thread-local keyed by `theme.name` (a `&'static str`). Per-frame lookups are
//! an O(1) array index into a `Copy` [`Palette`] — no color math, no allocation,
//! no lock. The whole `Palette` is ~400 bytes and copied by value.

use std::cell::RefCell;
use egui::Color32;

type Theme = crate::chart_renderer::gpu::Theme;

/// Semantic color slot — maps to a `Theme` base color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Accent,
    Bull,
    Bear,
    Warn,
    Text,
    Dim,
    /// Hairline borders / separators (`theme.toolbar_border`).
    Border,
    /// Chrome surface fill (`theme.toolbar_bg`).
    Surface,
    /// Canvas background (`theme.bg`).
    Bg,
}

impl Tone {
    pub(crate) const ALL: [Tone; 9] = [
        Tone::Accent, Tone::Bull, Tone::Bear, Tone::Warn, Tone::Text,
        Tone::Dim, Tone::Border, Tone::Surface, Tone::Bg,
    ];

    #[inline]
    fn base(self, t: &Theme) -> Color32 {
        match self {
            Tone::Accent  => t.accent,
            Tone::Bull    => t.bull,
            Tone::Bear    => t.bear,
            Tone::Warn    => t.warn,
            Tone::Text    => t.text,
            Tone::Dim     => t.dim,
            Tone::Border  => t.toolbar_border,
            Tone::Surface => t.toolbar_bg,
            Tone::Bg      => t.bg,
        }
    }

    #[inline]
    fn idx(self) -> usize {
        match self {
            Tone::Accent => 0, Tone::Bull => 1, Tone::Bear => 2, Tone::Warn => 3,
            Tone::Text => 4, Tone::Dim => 5, Tone::Border => 6, Tone::Surface => 7, Tone::Bg => 8,
        }
    }
}

/// Tailwind-style shade. `S500` is the base color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shade {
    S50, S100, S200, S300, S400, S500, S600, S700, S800, S900, S950,
}

impl Shade {
    #[inline]
    fn idx(self) -> usize {
        match self {
            Shade::S50 => 0, Shade::S100 => 1, Shade::S200 => 2, Shade::S300 => 3,
            Shade::S400 => 4, Shade::S500 => 5, Shade::S600 => 6, Shade::S700 => 7,
            Shade::S800 => 8, Shade::S900 => 9, Shade::S950 => 10,
        }
    }
}

/// Mix fraction toward white (negative) / black (positive) for each shade step.
/// 500 (index 5) is the base (0.0). Tuned to feel close to Tailwind ramps.
const MIX: [f32; 11] = [
    -0.82, -0.62, -0.42, -0.26, -0.12, 0.0, 0.12, 0.26, 0.42, 0.58, 0.74,
];

#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

#[inline]
fn mix(base: Color32, frac: f32) -> Color32 {
    // frac < 0 → toward white; frac > 0 → toward black.
    let (target, t) = if frac < 0.0 {
        (Color32::WHITE, -frac)
    } else {
        (Color32::BLACK, frac)
    };
    Color32::from_rgb(
        lerp_u8(base.r(), target.r(), t),
        lerp_u8(base.g(), target.g(), t),
        lerp_u8(base.b(), target.b(), t),
    )
}

/// 11-step precomputed shade ramp for one tone.
#[derive(Clone, Copy)]
pub struct Ramp([Color32; 11]);

impl Ramp {
    fn from_base(base: Color32) -> Self {
        let mut r = [base; 11];
        for (i, &frac) in MIX.iter().enumerate() {
            r[i] = mix(base, frac);
        }
        Ramp(r)
    }

    #[inline]
    pub fn at(&self, s: Shade) -> Color32 { self.0[s.idx()] }
}

/// All tone ramps for one theme. `Copy` (~400 bytes) so it is cheap to hand
/// around by value — built once, never mutated.
#[derive(Clone, Copy)]
pub struct Palette {
    ramps: [Ramp; 9],
}

impl Palette {
    fn from_theme(t: &Theme) -> Self {
        let mut ramps = [Ramp([Color32::TRANSPARENT; 11]); 9];
        for tone in Tone::ALL {
            ramps[tone.idx()] = Ramp::from_base(tone.base(t));
        }
        Palette { ramps }
    }

    /// The exact shade for a tone — O(1) array index.
    #[inline]
    pub fn shade(&self, tone: Tone, s: Shade) -> Color32 {
        self.ramps[tone.idx()].at(s)
    }

    /// The base (500) color for a tone.
    #[inline]
    pub fn base(&self, tone: Tone) -> Color32 {
        self.shade(tone, Shade::S500)
    }
}

thread_local! {
    /// Per-theme palette cache. Small (≤ ~12 themes), keyed by the theme's
    /// `&'static str` name so lookups never allocate after warmup.
    static PALETTE_CACHE: RefCell<Vec<(&'static str, Palette)>> = const { RefCell::new(Vec::new()) };
}

/// Get the cached [`Palette`] for `t`, building it on first use. Returns by
/// value (a cheap `Copy`) so the borrow on the cache is not held across the
/// caller's render — and there is no per-frame color math or allocation.
pub fn palette(t: &Theme) -> Palette {
    PALETTE_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        if let Some((_, p)) = cache.iter().find(|(name, _)| *name == t.name) {
            return *p;
        }
        let p = Palette::from_theme(t);
        cache.push((t.name, p));
        p
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luma(c: Color32) -> f32 {
        0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32
    }

    #[test]
    fn ramp_is_monotonic_light_to_dark() {
        // A mid-gray base produces a ramp that gets monotonically darker from
        // S50 → S950, with S500 == the base.
        let base = Color32::from_rgb(120, 130, 200);
        let r = Ramp::from_base(base);
        assert_eq!(r.at(Shade::S500), base, "500 must be the base color");
        let shades = [
            Shade::S50, Shade::S100, Shade::S200, Shade::S300, Shade::S400,
            Shade::S500, Shade::S600, Shade::S700, Shade::S800, Shade::S900, Shade::S950,
        ];
        for w in shades.windows(2) {
            let a = luma(r.at(w[0]));
            let b = luma(r.at(w[1]));
            assert!(a > b, "ramp must darken: {:?}({a}) -> {:?}({b})", w[0], w[1]);
        }
    }

    #[test]
    fn alpha_overlay_preserves_rgb() {
        let base = Color32::from_rgb(40, 200, 120);
        let r = Ramp::from_base(base);
        // S500 base used by Fill::Alpha — rgb stays, alpha applied separately.
        assert_eq!(r.at(Shade::S500), base);
    }
}
