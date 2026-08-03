//! DS-6.0 D2 — named design-system presets.
//!
//! The two-axis system is `ColorScheme × StyleSystem` — 22 × 9 combinations.
//! Six of those pairings are *designed*, and those six are what
//! `docs/handoffs/frontend-ds-adoption/12-T5-CERTIFICATION.md` certifies.
//! The rest are reachable but were never drawn by anyone.
//!
//! So the main UI offers these presets, and Theme Studio keeps the raw matrix
//! (decision D2). Curation where most users are, freedom where they go looking
//! for it — and it keeps the certification claim honest, because the presets
//! the app recommends are exactly the pairings that were certified.
//!
//! ## Why IDs and not indices
//!
//! The obvious encoding is the index pair the capture harness uses (`16:1` for
//! Aperture, `21:0` for Meridien). It is also a trap: those indices are
//! positions in two registries that grow when a user installs a theme pack.
//! Pinning a preset to a position means the preset silently starts pointing at
//! a different design the moment anything is inserted before it.
//!
//! Presets therefore name what they mean and resolve to positions at call time.
//! A preset whose scheme or style is missing resolves to `None` rather than to
//! the wrong thing.

/// A named, certified pairing of the two axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    /// Stable identifier — safe to persist.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// One-line description of the design's character, for the picker.
    pub blurb: &'static str,
    /// `ColorScheme` name to resolve on the palette axis.
    pub scheme: &'static str,
    /// `StyleSystem` name to resolve on the dimension axis.
    pub style: &'static str,
}

/// The six certified pairings, in the order the certification records them.
///
/// Lucid and Meridien deliberately share a palette — see `12-T5` §3: they ship
/// byte-identical colour and still read as different products, which is the
/// cleanest demonstration that the dimension axis carries weight on its own.
/// Keep them adjacent here; a picker that shows them side by side is showing
/// the strongest evidence the design system has.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "aperture",
        name: "Aperture",
        blurb: "Warm near-black, orange accent, soft radii",
        scheme: "aperture",
        style: "aperture",
    },
    Preset {
        id: "cadence",
        name: "Cadence",
        blurb: "Green accent, pill controls, dense screens",
        scheme: "cadence",
        style: "cadence",
    },
    Preset {
        id: "alto",
        name: "Alto",
        blurb: "Warm amber on near-black, tight rows",
        scheme: "alto",
        style: "alto",
    },
    Preset {
        id: "mariner",
        name: "Mariner",
        blurb: "Cool steel blue, precision markers",
        scheme: "mariner",
        style: "mariner",
    },
    Preset {
        id: "lucid",
        name: "Lucid",
        blurb: "Cream paper, rounded, airy editorial",
        scheme: "lucid",
        style: "lucid",
    },
    Preset {
        id: "meridien",
        name: "Meridien",
        blurb: "Same paper as Lucid — mono, uppercase, square",
        scheme: "meridien",
        style: "meridien",
    },
];

/// Look up a preset by its stable id.
pub fn by_id(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id.eq_ignore_ascii_case(id))
}

/// Resolve a preset to `(theme_idx, style_idx)` against the live registries.
///
/// Returns `None` if either axis is missing, rather than guessing — a preset
/// that cannot be resolved exactly is not a preset, and silently landing on a
/// neighbouring index would be worse than doing nothing.
pub fn resolve(p: &Preset, schemes: &[String], styles: &[String]) -> Option<(usize, usize)> {
    let ti = schemes.iter().position(|n| n.eq_ignore_ascii_case(p.scheme))?;
    let si = styles.iter().position(|n| n.eq_ignore_ascii_case(p.style))?;
    Some((ti, si))
}

/// Which preset, if any, the given axis pair currently corresponds to.
///
/// Used to show the active preset in the picker. Returns `None` when the user
/// has built a combination by hand in Theme Studio — which is a legitimate
/// state, not an error, and should read as "no preset" rather than snapping to
/// the nearest one.
pub fn active(schemes: &[String], styles: &[String], theme_idx: usize, style_idx: usize)
    -> Option<&'static Preset>
{
    PRESETS.iter().find(|p| resolve(p, schemes, styles) == Some((theme_idx, style_idx)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> { v.iter().map(|s| (*s).to_string()).collect() }

    #[test]
    fn resolves_by_name_not_position() {
        let schemes = names(&["dark", "aperture", "meridien"]);
        let styles  = names(&["meridien", "aperture"]);
        let ap = by_id("aperture").unwrap();
        assert_eq!(resolve(ap, &schemes, &styles), Some((1, 1)));

        // Insert a pack at the FRONT of both registries: the indices all shift,
        // and the preset must still land on Aperture. This is the whole reason
        // presets are stored as names — an index pair would now be wrong.
        let schemes2 = names(&["installed-pack", "dark", "aperture", "meridien"]);
        let styles2  = names(&["installed-pack", "meridien", "aperture"]);
        assert_eq!(resolve(ap, &schemes2, &styles2), Some((2, 2)));
    }

    #[test]
    fn missing_axis_resolves_to_none_not_a_guess() {
        let schemes = names(&["dark"]);
        let styles  = names(&["aperture"]);
        assert_eq!(resolve(by_id("aperture").unwrap(), &schemes, &styles), None);
    }

    #[test]
    fn hand_built_combination_reports_no_preset() {
        let schemes = names(&["aperture", "mariner"]);
        let styles  = names(&["aperture", "mariner"]);
        // A certified pairing reports itself.
        assert_eq!(active(&schemes, &styles, 0, 0).map(|p| p.id), Some("aperture"));
        // Mariner palette + Aperture dimensions is reachable from Theme Studio
        // but was never designed — it must read as "no preset", not snap to one.
        assert_eq!(active(&schemes, &styles, 1, 0), None);
    }

    #[test]
    fn every_preset_is_unique_and_addressable() {
        for p in PRESETS {
            assert_eq!(by_id(p.id).map(|q| q.id), Some(p.id), "preset {} not addressable", p.id);
        }
        let mut ids: Vec<_> = PRESETS.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n, "duplicate preset id");
    }

    /// The presets must be exactly the pairings `12-T5-CERTIFICATION` vouches
    /// for. If a preset is added without certifying it, the main UI starts
    /// recommending an undesigned combination.
    #[test]
    fn presets_match_the_certified_set() {
        let ids: Vec<_> = PRESETS.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            vec!["aperture", "cadence", "alto", "mariner", "lucid", "meridien"],
            "presets must match the six certified in 12-T5-CERTIFICATION"
        );
    }
}
