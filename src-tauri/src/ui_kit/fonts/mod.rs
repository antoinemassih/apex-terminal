//! Font registry — built-in families + runtime theme-supplied font bytes.
//!
//! ## Design
//!
//! All built-in font bytes are embedded at compile time (same `include_bytes!`
//! calls as the existing `icons::init_fonts` path). The registry adds a **name
//! → bytes** indirection layer so a theme pack can:
//!
//! 1. Register additional font bytes at startup/hot-reload time (via
//!    [`FontRegistry::register`]).
//! 2. Reference a family by the string names stored in
//!    `design_system::style_system::Typography` (`family_ui`, `family_mono`,
//!    `family_display`).
//! 3. Fall back gracefully — with a structured [`Warning`] — when a requested
//!    family is absent, so rendering never produces tofu or blank glyphs.
//!
//! ## What is NOT done here (host-integration step, deferred)
//!
//! This module **only builds `egui::FontDefinitions`**; it does NOT call
//! `ctx.set_fonts()`. The host (currently `icons::init_fonts` in the chart app)
//! is responsible for calling [`FontRegistry::build_definitions`] and pushing
//! the result to egui. That wire-up is a later integration step — we expose the
//! API; the host calls it. See the integration note at the bottom of this file.
//!
//! ## Family naming convention
//!
//! Family names match the human-readable names in `Typography`:
//!   - `"Inter"` → UI proportional (default)
//!   - `"JetBrains Mono"` → monospace (default)
//!   - `"Source Serif 4"` → display/serif
//!   - `"Plus Jakarta Sans"`, `"Space Grotesk"`, `"DM Sans"`, `"Geist"`,
//!     `"IBM Plex Sans"`, `"IBM Plex Mono"` — the additional picker families
//!
//! A theme that supplies no font data uses the built-in set (pixel-identical
//! to today). A theme that registers a new family name and sets
//! `Typography.family_ui = "My Custom Font"` will render the whole UI in that
//! font as soon as `build_definitions` is called and the context refreshed.

use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use std::collections::HashMap;
use std::sync::Arc;

// ── Warning type (S9 can later consume this) ─────────────────────────────────

/// A structured warning emitted by the font registry when a family lookup
/// falls back to the built-in default. Stream S9 (validation) can aggregate
/// these into its pack-validation report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// The family name that was requested but not found in the registry.
    pub requested_family: String,
    /// The family that was used instead.
    pub fallback_family:  String,
    /// Human-readable explanation for display in authoring tools / logs.
    pub message:          String,
}

impl Warning {
    fn missing(requested: &str, fallback: &str) -> Self {
        Self {
            requested_family: requested.to_owned(),
            fallback_family:  fallback.to_owned(),
            message: format!(
                "Font family \"{}\" is not registered; falling back to \"{}\".",
                requested, fallback
            ),
        }
    }
}

// ── FontSource ────────────────────────────────────────────────────────────────

/// The byte source for a single font face within a family.
#[derive(Clone)]
pub enum FontSource {
    /// Bytes embedded at compile time (built-in families).
    Static(&'static [u8]),
    /// Bytes provided at runtime (theme-supplied fonts).
    Dynamic(Arc<Vec<u8>>),
}

impl FontSource {
    fn as_bytes(&self) -> &[u8] {
        match self {
            FontSource::Static(b)  => b,
            FontSource::Dynamic(b) => b.as_slice(),
        }
    }
}

// ── FontFace ─────────────────────────────────────────────────────────────────

/// One font face within a family (a single TTF/OTF file).
#[derive(Clone)]
pub struct FontFace {
    /// Internal egui data-key (e.g. `"inter_regular"`).
    pub data_key: String,
    /// The font bytes.
    pub source:   FontSource,
    /// Render tweak (scale, y-offset) applied at load time.
    pub tweak:    FontTweak,
}

impl FontFace {
    fn new_static(key: &str, bytes: &'static [u8], tweak: FontTweak) -> Self {
        Self { data_key: key.to_owned(), source: FontSource::Static(bytes), tweak }
    }
}

// ── FamilyEntry ───────────────────────────────────────────────────────────────

/// A registered font family: one or more faces that are stacked (first = primary).
#[derive(Clone)]
pub struct FamilyEntry {
    /// Display name — the string used in `Typography.family_*`.
    pub name:  String,
    /// Faces in priority order (index 0 = primary face for the family).
    pub faces: Vec<FontFace>,
    /// Which egui built-in family slot to also slot this family into
    /// (`Proportional`, `Monospace`, or neither/`None` for named-only families).
    pub egui_slot: Option<FontFamily>,
}

// ── FontRegistry ─────────────────────────────────────────────────────────────

/// The font registry — built-in families plus runtime-registered theme fonts.
///
/// Construct with [`FontRegistry::new_with_builtins`] (returns the same families
/// that `icons::init_fonts` wires today).  Then call [`FontRegistry::register`]
/// to add theme-supplied bytes before [`FontRegistry::build_definitions`].
pub struct FontRegistry {
    /// Map of family display-name → entry.
    families: HashMap<String, FamilyEntry>,
    /// Default UI (proportional) family name — used as fallback.
    default_ui:      String,
    /// Default monospace family name — used as fallback.
    default_mono:    String,
    /// Default display/hero family name — used as fallback.
    default_display: String,
}

// ── Built-in tweaks ───────────────────────────────────────────────────────────

fn tweak_mono() -> FontTweak {
    FontTweak { scale: 1.0, y_offset_factor: -0.02, y_offset: 0.0, baseline_offset_factor: 0.0 }
}

fn tweak_sans() -> FontTweak {
    FontTweak { scale: 1.02, y_offset_factor: -0.01, y_offset: 0.0, baseline_offset_factor: 0.0 }
}

fn tweak_none() -> FontTweak {
    FontTweak { scale: 1.0, y_offset_factor: 0.0, y_offset: 0.0, baseline_offset_factor: 0.0 }
}

impl FontRegistry {
    // ── Constants: well-known built-in family names ───────────────────────────

    /// The default UI proportional family — matches `Typography::default_family_ui()`.
    pub const DEFAULT_UI:      &'static str = "Inter";
    /// The default monospace family — matches `Typography::default_family_mono()`.
    pub const DEFAULT_MONO:    &'static str = "JetBrains Mono";
    /// The default display/hero family — matches `Typography::default_family_display()`.
    pub const DEFAULT_DISPLAY: &'static str = "Inter";

    /// Build the registry pre-loaded with all fonts that `icons::init_fonts`
    /// currently embeds.  The result is font-for-font identical to the existing
    /// setup; no visual change occurs until a caller activates a different family
    /// via [`FontRegistry::build_definitions`].
    pub fn new_with_builtins() -> Self {
        let mut r = Self {
            families:        HashMap::new(),
            default_ui:      Self::DEFAULT_UI.to_owned(),
            default_mono:    Self::DEFAULT_MONO.to_owned(),
            default_display: Self::DEFAULT_DISPLAY.to_owned(),
        };

        // ── JetBrains Mono (default monospace) ───────────────────────────────
        r.insert(FamilyEntry {
            name:      "JetBrains Mono".to_owned(),
            egui_slot: Some(FontFamily::Monospace),
            faces: vec![
                FontFace::new_static("jetbrains_mono",
                    include_bytes!("../JetBrainsMono-Regular.ttf"), tweak_mono()),
                FontFace::new_static("jetbrains_mono_bold",
                    include_bytes!("../JetBrainsMono-Bold.ttf"), tweak_mono()),
            ],
        });

        // ── Inter (default proportional / UI family) ──────────────────────────
        r.insert(FamilyEntry {
            name:      "Inter".to_owned(),
            egui_slot: Some(FontFamily::Proportional),
            faces: vec![
                FontFace::new_static("inter",
                    include_bytes!("../Inter-Medium.ttf"), tweak_sans()),
                FontFace::new_static("inter_regular",
                    include_bytes!("../Inter-Regular.ttf"), tweak_sans()),
                FontFace::new_static("inter_semibold",
                    include_bytes!("../Inter-SemiBold.ttf"), tweak_sans()),
                FontFace::new_static("inter_bold",
                    include_bytes!("../Inter-Bold.ttf"), tweak_sans()),
            ],
        });

        // ── Plus Jakarta Sans ─────────────────────────────────────────────────
        r.insert(FamilyEntry {
            name:      "Plus Jakarta Sans".to_owned(),
            egui_slot: None,
            faces: vec![
                FontFace::new_static("plus_jakarta",
                    include_bytes!("../PlusJakartaSans-Medium.ttf"), tweak_sans()),
            ],
        });

        // ── Space Grotesk ─────────────────────────────────────────────────────
        r.insert(FamilyEntry {
            name:      "Space Grotesk".to_owned(),
            egui_slot: None,
            faces: vec![
                FontFace::new_static("space_grotesk",
                    include_bytes!("../SpaceGrotesk-Medium.ttf"), tweak_sans()),
            ],
        });

        // ── DM Sans ───────────────────────────────────────────────────────────
        r.insert(FamilyEntry {
            name:      "DM Sans".to_owned(),
            egui_slot: None,
            faces: vec![
                FontFace::new_static("dm_sans",
                    include_bytes!("../DMSans-Medium.ttf"), tweak_sans()),
            ],
        });

        // ── Geist ─────────────────────────────────────────────────────────────
        r.insert(FamilyEntry {
            name:      "Geist".to_owned(),
            egui_slot: None,
            faces: vec![
                FontFace::new_static("geist",
                    include_bytes!("../Geist-Medium.ttf"), tweak_sans()),
            ],
        });

        // ── IBM Plex Sans ─────────────────────────────────────────────────────
        r.insert(FamilyEntry {
            name:      "IBM Plex Sans".to_owned(),
            egui_slot: None,
            faces: vec![
                FontFace::new_static("ibm_plex_sans",
                    include_bytes!("../IBMPlexSans-Regular.ttf"), tweak_sans()),
                FontFace::new_static("ibm_plex_sans_sb",
                    include_bytes!("../IBMPlexSans-SemiBold.ttf"), tweak_sans()),
            ],
        });

        // ── IBM Plex Mono ─────────────────────────────────────────────────────
        r.insert(FamilyEntry {
            name:      "IBM Plex Mono".to_owned(),
            egui_slot: None,
            faces: vec![
                FontFace::new_static("ibm_plex_mono",
                    include_bytes!("../IBMPlexMono-Regular.ttf"), tweak_mono()),
                FontFace::new_static("ibm_plex_mono_bold",
                    include_bytes!("../IBMPlexMono-Bold.ttf"), tweak_mono()),
            ],
        });

        // ── Source Serif 4 (display / serif) ──────────────────────────────────
        r.insert(FamilyEntry {
            name:      "Source Serif 4".to_owned(),
            egui_slot: Some(FontFamily::Name("serif".into())),
            faces: vec![
                FontFace::new_static("source_serif4_regular",
                    include_bytes!("../SourceSerif4-Regular.ttf"), tweak_none()),
                FontFace::new_static("source_serif4_bold",
                    include_bytes!("../SourceSerif4-Bold.ttf"), tweak_none()),
            ],
        });

        r
    }

    /// Insert or replace a family entry.
    fn insert(&mut self, entry: FamilyEntry) {
        self.families.insert(entry.name.clone(), entry);
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Register a theme-supplied font family by name and runtime byte vector.
    ///
    /// If the registry already has a family with this name, it is replaced
    /// (allowing a theme to override even a built-in family if desired).
    ///
    /// `data_key` is the egui-internal font data key; it must be unique across
    /// all registered faces.  Suggestion: `"theme_<family_slug>_regular"`.
    ///
    /// `egui_slot` controls whether this family is also inserted into the egui
    /// built-in `Proportional`/`Monospace` lists.  Pass `None` for named-only.
    pub fn register(
        &mut self,
        name:      impl Into<String>,
        data_key:  impl Into<String>,
        bytes:     Vec<u8>,
        tweak:     FontTweak,
        egui_slot: Option<FontFamily>,
    ) {
        let name = name.into();
        self.families.insert(name.clone(), FamilyEntry {
            name,
            egui_slot,
            faces: vec![FontFace {
                data_key: data_key.into(),
                source:   FontSource::Dynamic(Arc::new(bytes)),
                tweak,
            }],
        });
    }

    /// Remove a previously registered theme-supplied family by name.  Built-in
    /// families can be removed (the caller's responsibility to ensure fallback).
    pub fn unregister(&mut self, name: &str) {
        self.families.remove(name);
    }

    /// Returns `true` if a family with this name is currently registered.
    pub fn contains(&self, name: &str) -> bool {
        self.families.contains_key(name)
    }

    /// Resolve which actual family name to use for `requested`, falling back to
    /// `fallback_name` if `requested` is not registered.  Pushes a [`Warning`]
    /// into `warnings` on fallback.
    ///
    /// Returns the resolved family name (the one that IS registered).
    pub fn resolve_family<'a>(
        &self,
        requested:     &'a str,
        fallback_name: &'a str,
        warnings:      &mut Vec<Warning>,
    ) -> &'a str {
        if self.families.contains_key(requested) {
            requested
        } else {
            warnings.push(Warning::missing(requested, fallback_name));
            fallback_name
        }
    }

    /// Build an `egui::FontDefinitions` from the registry, applying the active
    /// family selection from the supplied names.
    ///
    /// - `family_ui`      — the name to use as the proportional UI family.
    /// - `family_mono`    — the name to use as the monospace family.
    /// - `family_display` — the name to use for the `"serif"` named slot.
    ///
    /// Any name not found in the registry falls back to the built-in default
    /// and a [`Warning`] is appended to the returned `Vec<Warning>`.
    ///
    /// **After calling this, the host must pass the result to `ctx.set_fonts()`.**
    /// This module does NOT call `ctx.set_fonts()` — that remains the host's
    /// responsibility (see module-level doc).
    ///
    /// Phosphor icon fonts are registered separately by the caller via
    /// `egui_phosphor::add_to_fonts(...)`.
    pub fn build_definitions(
        &self,
        family_ui:      &str,
        family_mono:    &str,
        family_display: &str,
    ) -> (FontDefinitions, Vec<Warning>) {
        let mut warnings = Vec::new();

        let ui_name      = self.resolve_family(family_ui,      &self.default_ui,      &mut warnings);
        let mono_name    = self.resolve_family(family_mono,    &self.default_mono,    &mut warnings);
        let display_name = self.resolve_family(family_display, &self.default_display, &mut warnings);

        let mut defs = FontDefinitions::default();

        // Insert ALL registered font data into egui (so named families work
        // even if not slotted into Proportional/Monospace).
        for entry in self.families.values() {
            for face in &entry.faces {
                let data = match &face.source {
                    FontSource::Static(bytes) =>
                        FontData::from_static(bytes).tweak(face.tweak),
                    FontSource::Dynamic(bytes) =>
                        FontData::from_owned(bytes.as_ref().clone()).tweak(face.tweak),
                };
                defs.font_data.insert(face.data_key.clone(), Arc::new(data));
            }
        }

        // Wire the named egui families.  We do this in a specific priority order:
        // 1. Named families for weight variants (inter_regular / inter_semibold / etc.)
        self.wire_named_families(&mut defs);

        // 2. Slot the active UI family into Proportional (at position 0).
        if let Some(entry) = self.families.get(ui_name) {
            let primary_key = entry.faces[0].data_key.clone();
            if let Some(prop) = defs.families.get_mut(&FontFamily::Proportional) {
                prop.retain(|k| k != &primary_key);
                prop.insert(0, primary_key);
            }
        }

        // 3. Slot the active mono family into Monospace (at position 0).
        //    JetBrains Mono ALWAYS leads the Monospace family for financial data
        //    digit alignment — if the requested mono != JetBrains Mono, JetBrains
        //    is inserted first as a secondary fallback.
        if let Some(entry) = self.families.get(mono_name) {
            let primary_key = entry.faces[0].data_key.clone();
            if let Some(mono) = defs.families.get_mut(&FontFamily::Monospace) {
                mono.retain(|k| k != &primary_key);
                mono.insert(0, primary_key);
            }
        }
        // Ensure JetBrains Mono is always present in Monospace for digit alignment.
        if mono_name != Self::DEFAULT_MONO {
            if let Some(entry) = self.families.get(Self::DEFAULT_MONO) {
                let jb_key = entry.faces[0].data_key.clone();
                if let Some(mono) = defs.families.get_mut(&FontFamily::Monospace) {
                    if !mono.contains(&jb_key) {
                        mono.insert(1, jb_key); // slot 1 so custom mono leads
                    }
                }
            }
        }

        // 4. Slot the display family into the "serif" named slot.
        if let Some(entry) = self.families.get(display_name) {
            let face_keys: Vec<String> = entry.faces.iter().map(|f| f.data_key.clone()).collect();
            defs.families.insert(FontFamily::Name("serif".into()), face_keys);
        }

        (defs, warnings)
    }

    /// Wire all named-weight sub-families (inter_regular / inter_semibold /
    /// jetbrains_mono_bold / etc.) so call-sites using
    /// `RichText::family(FontFamily::Name("inter_semibold".into()))` keep working.
    fn wire_named_families(&self, defs: &mut FontDefinitions) {
        // Named sub-families that must always be available as explicit family slots.
        // These mirror what `icons::init_fonts` previously did manually.
        const NAMED_SINGLETONS: &[(&str, &str)] = &[
            ("inter_regular",       "inter_regular"),
            ("inter_semibold",      "inter_semibold"),
            ("inter_bold",          "inter_bold"),
            ("jetbrains_mono_bold", "jetbrains_mono_bold"),
        ];
        for (family_slot, data_key) in NAMED_SINGLETONS {
            if defs.font_data.contains_key(*data_key) {
                defs.families.insert(
                    FontFamily::Name((*family_slot).into()),
                    vec![(*data_key).to_owned()],
                );
            }
        }
    }

    /// Convenience: build definitions from a `Typography` struct, using its
    /// `family_ui`, `family_mono`, and `family_display` fields as the active
    /// selection.  Returns the definitions and any fallback warnings.
    ///
    /// This is the primary entry point for the host integration step.
    pub fn build_from_typography(
        &self,
        typography: &crate::design_system::style_system::Typography,
    ) -> (FontDefinitions, Vec<Warning>) {
        self.build_definitions(
            &typography.family_ui,
            &typography.family_mono,
            &typography.family_display,
        )
    }
}

// ── Integration note (deferred host wiring) ───────────────────────────────────
//
// To integrate with the live egui context, the host (`native_main.rs` or
// wherever `icons::init_fonts` is called today) should:
//
//   1. Create a `FontRegistry::new_with_builtins()`.
//   2. Optionally call `registry.register(...)` for theme-supplied fonts.
//   3. Call `egui_phosphor::add_to_fonts(&mut defs, ...)` on the result.
//   4. Ensure phosphor is a fallback in the Monospace family.
//   5. Call `ctx.set_fonts(defs)`.
//
// Example:
//
//   let mut registry = FontRegistry::new_with_builtins();
//   let typography   = active_style_system.typography.clone();
//   let (mut defs, warnings) = registry.build_from_typography(&typography);
//   egui_phosphor::add_to_fonts(&mut defs, egui_phosphor::Variant::Regular);
//   egui_phosphor::add_to_fonts(&mut defs, egui_phosphor::Variant::Bold);
//   egui_phosphor::add_to_fonts(&mut defs, egui_phosphor::Variant::Fill);
//   if let Some(mono) = defs.families.get_mut(&egui::FontFamily::Monospace) {
//       if !mono.contains(&"phosphor".to_string()) { mono.push("phosphor".into()); }
//   }
//   ctx.set_fonts(defs);
//   for w in &warnings { log::warn!("{}", w.message); }
//
// The existing `icons::init_fonts` function remains intact and is the current
// live path.  The registry API is additive — no existing behavior is changed
// until the host is explicitly updated to call it.
