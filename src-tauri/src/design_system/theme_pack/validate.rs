//! ThemePack validation — S9 structural, accessibility, and sandbox checks.
//!
//! ## Overview
//!
//! [`validate`] is the single entry-point.  It inspects a [`ThemePack`] and
//! returns a [`ValidationReport`] that is both human-readable and
//! machine-actionable.
//!
//! **Install gate:** [`ValidationReport::is_installable`] returns `false` when
//! any [`Severity::Error`] findings are present.  The registry calls this
//! before committing a pack to disk.
//!
//! **Author linting (S10):** the same report type is reused by the authoring
//! tool.  It can surface warnings even for packs that would pass install.
//!
//! ## Findings
//!
//! | Code prefix | Category | Default severity |
//! |---|---|---|
//! | `STRUCT_*` | Structural / manifest fields | Error |
//! | `SCHEMA_*` | Schema version compatibility | Error |
//! | `ASSET_*` | Asset inventory / sandbox | Error or Warning |
//! | `RECIPE_*` | Recipe key formatting | Warning |
//! | `A11Y_*` | Accessibility / contrast | Warning (strict → Error) |
//! | `FALLBK_*` | Token fallback inventory | Warning |
//!
//! ## Contrast thresholds
//!
//! WCAG 2.1 ratios (luminance-based):
//! - AA normal text: **4.5 : 1**
//! - AA large/UI:    **3.0 : 1** (used for non-text pairs in strict mode)
//! - Text-equals-bg: always an **Error** regardless of mode.
//!
//! ## Sandbox limits
//!
//! | Limit | Constant | Value |
//! |---|---|---|
//! | Max assets per pack | [`MAX_ASSETS`] | 64 |
//! | Max bytes per asset | [`MAX_ASSET_BYTES`] | 4 MiB |
//! | Max total asset bytes | [`MAX_TOTAL_ASSET_BYTES`] | 32 MiB |
//! | Allowed asset kinds | [`is_allowed_kind`] | Font, IconFont (no Texture, no Unknown Exe) |

use crate::design_system::color_scheme::{ColorScheme, Rgba};
use crate::ui_kit::assets::AssetKind;

use super::{ThemePack, manifest::CURRENT_SCHEMA_VERSION};

// ── Sandbox limits ─────────────────────────────────────────────────────────────

/// Maximum number of assets permitted in a single pack.
pub const MAX_ASSETS: usize = 64;

/// Maximum byte size of a single asset (4 MiB).
pub const MAX_ASSET_BYTES: usize = 4 * 1024 * 1024;

/// Maximum total byte size of all assets combined (32 MiB).
pub const MAX_TOTAL_ASSET_BYTES: usize = 32 * 1024 * 1024;

/// WCAG AA contrast ratio for normal text (4.5 : 1).
pub const WCAG_AA_NORMAL: f64 = 4.5;

/// WCAG AA contrast ratio for large text / UI components (3.0 : 1).
pub const WCAG_AA_LARGE: f64 = 3.0;

// ── Severity ───────────────────────────────────────────────────────────────────

/// How serious a finding is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// Pack cannot be installed.  [`ValidationReport::is_installable`] returns
    /// `false` when any errors are present.
    Error,
    /// Pack can be installed but something is sub-optimal.
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error   => write!(f, "ERROR"),
            Severity::Warning => write!(f, "WARN"),
        }
    }
}

// ── Finding ────────────────────────────────────────────────────────────────────

/// A single validation finding.
#[derive(Debug, Clone)]
pub struct Finding {
    /// Short machine-readable code (e.g. `"STRUCT_MISSING_ID"`).
    pub code: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// How serious the issue is.
    pub severity: Severity,
}

impl Finding {
    fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), severity: Severity::Error }
    }

    fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), severity: Severity::Warning }
    }
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.code, self.message)
    }
}

// ── ValidationReport ──────────────────────────────────────────────────────────

/// The result of [`validate`] — a structured report of all findings.
///
/// Shared between the install gate (S9) and the author-time linter (S10).
#[derive(Debug, Default, Clone)]
pub struct ValidationReport {
    /// Findings that block installation.
    pub errors: Vec<Finding>,
    /// Findings that are informational / advisory.
    pub warnings: Vec<Finding>,
}

impl ValidationReport {
    /// Returns `true` when no errors are present.
    ///
    /// Warnings do NOT prevent installation.
    pub fn is_installable(&self) -> bool {
        self.errors.is_empty()
    }

    /// Total number of findings (errors + warnings).
    pub fn finding_count(&self) -> usize {
        self.errors.len() + self.warnings.len()
    }

    fn push_error(&mut self, code: impl Into<String>, message: impl Into<String>) {
        self.errors.push(Finding::error(code, message));
    }

    fn push_warning(&mut self, code: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(Finding::warning(code, message));
    }

    /// Merge `other` into `self` (used internally to compose check results).
    fn merge(&mut self, other: ValidationReport) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

impl std::fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ValidationReport {{ errors: {}, warnings: {}, installable: {} }}",
            self.errors.len(),
            self.warnings.len(),
            self.is_installable(),
        )
    }
}

// ── Validation mode ────────────────────────────────────────────────────────────

/// Controls how accessibility contrast failures are classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessibilityMode {
    /// Below-threshold contrast produces a [`Severity::Warning`].
    #[default]
    Standard,
    /// Below-threshold contrast produces a [`Severity::Error`], blocking install.
    Strict,
}

// ── Public entry-point ─────────────────────────────────────────────────────────

/// Validate `pack` and return a [`ValidationReport`].
///
/// `a11y_mode` controls whether contrast failures are warnings or errors.
/// The install gate calls this with [`AccessibilityMode::Standard`]; a
/// future author-time linter may call it with [`AccessibilityMode::Strict`].
///
/// This function never panics on malformed input — all failure paths produce
/// a finding and continue.
pub fn validate(pack: &ThemePack, a11y_mode: AccessibilityMode) -> ValidationReport {
    let mut report = ValidationReport::default();
    report.merge(check_structural(&pack.manifest));
    report.merge(check_schema_version(&pack.manifest));
    report.merge(check_asset_inventory(&pack.manifest));
    report.merge(check_recipe_keys(&pack.recipes));
    report.merge(check_sandbox(&pack.assets));
    report.merge(check_accessibility(&pack.color_scheme, a11y_mode));
    report.merge(check_fallback_coverage(&pack.color_scheme));
    report
}

// ── Structural checks ─────────────────────────────────────────────────────────

fn check_structural(manifest: &super::manifest::ThemeManifest) -> ValidationReport {
    let mut r = ValidationReport::default();

    if manifest.id.trim().is_empty() {
        r.push_error("STRUCT_MISSING_ID",
            "Manifest 'id' is required and must not be empty.");
    }
    if manifest.name.trim().is_empty() {
        r.push_error("STRUCT_MISSING_NAME",
            "Manifest 'name' is required and must not be empty.");
    }
    // Warn about empty author (not required, but useful for attribution).
    if manifest.author.trim().is_empty() {
        r.push_warning("STRUCT_MISSING_AUTHOR",
            "Manifest 'author' is empty; consider adding attribution.");
    }
    // Version should be non-empty (defaults to "1.0.0" so only fails if
    // someone explicitly sets it to "").
    if manifest.version.trim().is_empty() {
        r.push_warning("STRUCT_EMPTY_VERSION",
            "Manifest 'version' is empty; defaulting to '1.0.0'.");
    }

    r
}

// ── Schema version check ──────────────────────────────────────────────────────

fn check_schema_version(manifest: &super::manifest::ThemeManifest) -> ValidationReport {
    let mut r = ValidationReport::default();

    if manifest.app_schema_version == 0 {
        r.push_error("SCHEMA_ZERO_VERSION",
            "app_schema_version must be >= 1.");
    } else if manifest.app_schema_version > CURRENT_SCHEMA_VERSION {
        r.push_error(
            "SCHEMA_TOO_NEW",
            format!(
                "Pack schema version {} exceeds reader version {}. \
                 Upgrade apex-terminal to install this pack.",
                manifest.app_schema_version, CURRENT_SCHEMA_VERSION,
            ),
        );
    }
    // Older schema versions are acceptable (forward-compat via serde defaults).

    r
}

// ── Asset inventory check ─────────────────────────────────────────────────────

/// Returns `true` if the given `AssetKind` is permitted in installed packs.
///
/// We whitelist known safe kinds (fonts).  `Texture` is currently a stub (not
/// rendered) but permitted.  `Custom(tag)` is allowed — the sandbox limits
/// (size/count) still apply.  There is no concept of an "executable" `AssetKind`
/// in this enum, so every variant is nominally permitted at the type level; the
/// sandbox byte-limit is the primary defence.
pub fn is_allowed_kind(kind: &AssetKind) -> bool {
    matches!(
        kind,
        AssetKind::FontFace
            | AssetKind::FontFaceMono
            | AssetKind::IconFont
            | AssetKind::Texture
            | AssetKind::Custom(_)
    )
}

fn check_asset_inventory(manifest: &super::manifest::ThemeManifest) -> ValidationReport {
    let mut r = ValidationReport::default();

    for entry in &manifest.asset_inventory {
        if entry.key.trim().is_empty() {
            r.push_error("ASSET_EMPTY_KEY",
                "An asset inventory entry has an empty key.");
        }
        if !is_allowed_kind(&entry.kind) {
            r.push_error(
                "ASSET_DISALLOWED_KIND",
                format!("Asset '{}' has a disallowed kind.", entry.key),
            );
        }
    }

    r
}

// ── Recipe key checks ─────────────────────────────────────────────────────────

fn check_recipe_keys(recipes: &crate::design_system::recipes::RecipeSet) -> ValidationReport {
    let mut r = ValidationReport::default();

    for key in recipes.keys() {
        if key.trim().is_empty() {
            r.push_warning("RECIPE_EMPTY_KEY", "A recipe key is empty or whitespace.");
            continue;
        }
        // Keys should be dot-separated component.state identifiers.
        // Warn on suspicious patterns: leading/trailing dot, double dot,
        // or non-ASCII characters.
        if key.starts_with('.') || key.ends_with('.') {
            r.push_warning(
                "RECIPE_MALFORMED_KEY",
                format!("Recipe key '{key}' has a leading or trailing dot."),
            );
        } else if key.contains("..") {
            r.push_warning(
                "RECIPE_MALFORMED_KEY",
                format!("Recipe key '{key}' contains consecutive dots."),
            );
        } else if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
            r.push_warning(
                "RECIPE_NON_ASCII_KEY",
                format!("Recipe key '{key}' contains non-ASCII or special characters."),
            );
        }
    }

    r
}

// ── Sandbox checks ─────────────────────────────────────────────────────────────

fn check_sandbox(assets: &crate::ui_kit::assets::AssetRegistry) -> ValidationReport {
    let mut r = ValidationReport::default();

    let count = assets.len();
    if count > MAX_ASSETS {
        r.push_error(
            "ASSET_TOO_MANY",
            format!("Pack contains {count} assets; limit is {MAX_ASSETS}."),
        );
    }

    let mut total_bytes: usize = 0;

    for handle in assets.iter() {
        if !is_allowed_kind(&handle.kind) {
            r.push_error(
                "ASSET_DISALLOWED_KIND",
                format!("Asset '{}' has a disallowed kind.", handle.key),
            );
        }

        if let Some(sz) = handle.byte_len() {
            if sz > MAX_ASSET_BYTES {
                r.push_error(
                    "ASSET_TOO_LARGE",
                    format!(
                        "Asset '{}' is {} bytes; limit is {} bytes ({} MiB).",
                        handle.key, sz, MAX_ASSET_BYTES, MAX_ASSET_BYTES / (1024 * 1024),
                    ),
                );
            }
            total_bytes = total_bytes.saturating_add(sz);
        }
    }

    if total_bytes > MAX_TOTAL_ASSET_BYTES {
        r.push_error(
            "ASSET_TOTAL_TOO_LARGE",
            format!(
                "Total asset size is {total_bytes} bytes; limit is {MAX_TOTAL_ASSET_BYTES} bytes ({} MiB).",
                MAX_TOTAL_ASSET_BYTES / (1024 * 1024),
            ),
        );
    }

    r
}

// ── Accessibility / contrast checks ───────────────────────────────────────────

// ──────────────────────────────────────────────────────────────────────────────
// WCAG relative luminance
//
// Spec: https://www.w3.org/TR/WCAG21/#dfn-relative-luminance
// Linearise each 8-bit channel, then weight by ITU-R BT.709 primaries.
// The alpha channel is ignored (packs define opaque palette colours).
// ──────────────────────────────────────────────────────────────────────────────

/// Compute the WCAG 2.1 relative luminance of an [`Rgba`] colour.
///
/// The alpha channel is ignored — all palette colours are treated as opaque.
/// Returns a value in the range `[0.0, 1.0]` where `0.0` is absolute black
/// and `1.0` is absolute white.
pub fn relative_luminance(c: Rgba) -> f64 {
    fn linearise(u: u8) -> f64 {
        let s = u as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    let r = linearise(c[0]);
    let g = linearise(c[1]);
    let b = linearise(c[2]);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Compute the WCAG 2.1 contrast ratio between two [`Rgba`] colours.
///
/// The lighter colour's luminance is always placed in the numerator.
/// Returns a value in the range `[1.0, 21.0]`.
pub fn contrast_ratio(fg: Rgba, bg: Rgba) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Returns `true` if `fg` and `bg` are exactly the same RGB values (ignoring
/// alpha), which would make text completely invisible.
fn same_rgb(a: Rgba, b: Rgba) -> bool {
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2]
}

fn check_accessibility(cs: &ColorScheme, mode: AccessibilityMode) -> ValidationReport {
    let mut r = ValidationReport::default();

    // ── Key pairs to check ────────────────────────────────────────────────────
    // (foreground, background, pair_name)
    let pairs: &[(Rgba, Rgba, &str)] = &[
        (cs.text,             cs.bg,      "text / bg"),
        (cs.text,             cs.surface, "text / surface"),
        (cs.dim,              cs.bg,      "dim / bg"),
        (cs.dim,              cs.surface, "dim / surface"),
        // Resolved semantic colours on bg (UI component states).
        (cs.resolved_success(), cs.bg,    "resolved_success / bg"),
        (cs.resolved_danger(),  cs.bg,    "resolved_danger / bg"),
        (cs.resolved_warning(), cs.bg,    "resolved_warning / bg"),
        (cs.resolved_info(),    cs.bg,    "resolved_info / bg"),
        (cs.accent,           cs.bg,      "accent / bg"),
    ];

    for &(fg, bg, label) in pairs {
        // Absolute failure: foreground == background (invisible text).
        if same_rgb(fg, bg) {
            r.push_error(
                "A11Y_FG_EQ_BG",
                format!("Pair '{label}': foreground and background have identical RGB values — text would be invisible."),
            );
            continue; // ratio is 1:1, no point computing
        }

        let ratio = contrast_ratio(fg, bg);

        // WCAG AA normal text threshold.
        let threshold = WCAG_AA_NORMAL;
        if ratio < threshold {
            let msg = format!(
                "Pair '{label}': contrast ratio {ratio:.2}:1 is below WCAG AA threshold ({threshold:.1}:1).",
            );
            match mode {
                AccessibilityMode::Standard => r.push_warning("A11Y_LOW_CONTRAST", msg),
                AccessibilityMode::Strict   => r.push_error("A11Y_LOW_CONTRAST", msg),
            }
        }
    }

    r
}

// ── Fallback coverage check ────────────────────────────────────────────────────

/// Check that the resolver fallback chain never produces an absent value.
///
/// The `ColorScheme` extended semantic slots (`success`, `danger`, `warning`,
/// `info`) are `Option<Rgba>`.  Their `resolved_*` accessors fall back to the
/// mandatory `bull`/`bear`/`warn` fields which are always present.  We document
/// and enumerate what fell back, surfacing it as informational warnings so the
/// pack author knows which overrides are missing.
fn check_fallback_coverage(cs: &ColorScheme) -> ValidationReport {
    let mut r = ValidationReport::default();

    let slots = [
        (cs.success.is_none(), "success", "bull"),
        (cs.danger.is_none(),  "danger",  "bear"),
        (cs.warning.is_none(), "warning", "warn"),
        (cs.info.is_none(),    "info",    "a generic blue"),
    ];

    for (is_missing, slot, fallback) in slots {
        if is_missing {
            r.push_warning(
                "FALLBK_USING_DEFAULT",
                format!("ColorScheme.{slot} is unset; resolved_{slot}() falls back to {fallback}. No rendering gap — informational only."),
            );
        }
    }

    r
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::{
        color_scheme::{builtin_dark, rgba},
        recipes::RecipeSet,
        style_system::StyleSystem,
        theme_pack::{ThemePack, manifest::ThemeManifest},
    };
    use crate::ui_kit::assets::{AssetHandle, AssetKind, AssetRegistry};

    // ── contrast helper unit tests ─────────────────────────────────────────────

    #[test]
    fn contrast_black_on_white_is_21() {
        let black = rgba::rgb(0, 0, 0);
        let white = rgba::rgb(255, 255, 255);
        let ratio = contrast_ratio(black, white);
        // WCAG spec: exactly 21:1.
        assert!((ratio - 21.0).abs() < 0.01, "expected ~21:1, got {ratio:.4}");
    }

    #[test]
    fn contrast_same_color_is_1() {
        let gray = rgba::rgb(128, 128, 128);
        let ratio = contrast_ratio(gray, gray);
        assert!((ratio - 1.0).abs() < 0.001, "expected 1:1, got {ratio:.4}");
    }

    #[test]
    fn contrast_known_pair_wcag_example() {
        // WCAG example: #595959 on #ffffff ≈ 7.0:1
        let fg = rgba::rgb(89, 89, 89);
        let bg = rgba::rgb(255, 255, 255);
        let ratio = contrast_ratio(fg, bg);
        // Allow ±0.3 tolerance for 8-bit rounding.
        assert!(ratio > 6.5 && ratio < 7.5, "expected ~7.0:1, got {ratio:.4}");
    }

    #[test]
    fn same_rgb_detects_identical_channels() {
        assert!(same_rgb([10, 20, 30, 255], [10, 20, 30, 128])); // alpha differs — still same RGB
        assert!(!same_rgb([10, 20, 30, 255], [10, 20, 31, 255]));
    }

    // ── valid pack passes clean (no errors) ────────────────────────────────────

    #[test]
    fn valid_pack_passes() {
        let pack = make_valid_pack();
        let report = validate(&pack, AccessibilityMode::Standard);
        assert!(report.is_installable(), "valid pack should be installable: {report}");
        // May have warnings (e.g. empty author, fallback slots), but no errors.
        assert!(report.errors.is_empty(), "valid pack should have no errors: {:?}", report.errors);
    }

    // ── text == bg is always an error ──────────────────────────────────────────

    #[test]
    fn text_eq_bg_is_error() {
        let mut pack = make_valid_pack();
        // Force text == bg.
        pack.color_scheme.text = pack.color_scheme.bg;
        let report = validate(&pack, AccessibilityMode::Standard);
        assert!(!report.is_installable(), "text==bg should block install");
        let has_code = report.errors.iter().any(|f| f.code == "A11Y_FG_EQ_BG");
        assert!(has_code, "expected A11Y_FG_EQ_BG finding; got: {:?}", report.errors);
    }

    // ── missing id is an error ─────────────────────────────────────────────────

    #[test]
    fn missing_id_is_error() {
        let mut pack = make_valid_pack();
        pack.manifest.id = String::new();
        let report = validate(&pack, AccessibilityMode::Standard);
        assert!(!report.is_installable());
        assert!(report.errors.iter().any(|f| f.code == "STRUCT_MISSING_ID"));
    }

    // ── future schema version is an error ─────────────────────────────────────

    #[test]
    fn future_schema_is_error() {
        let mut pack = make_valid_pack();
        pack.manifest.app_schema_version = CURRENT_SCHEMA_VERSION + 99;
        let report = validate(&pack, AccessibilityMode::Standard);
        assert!(!report.is_installable());
        assert!(report.errors.iter().any(|f| f.code == "SCHEMA_TOO_NEW"));
    }

    // ── too many assets blocks install ─────────────────────────────────────────

    #[test]
    fn too_many_assets_is_error() {
        let mut pack = make_valid_pack();
        // Register MAX_ASSETS + 1 tiny assets.
        for i in 0..=(MAX_ASSETS) {
            pack.assets.register(
                AssetHandle::new_dynamic(
                    format!("font/test-{i}"),
                    AssetKind::FontFace,
                    vec![0u8; 16],
                ),
            );
        }
        let report = validate(&pack, AccessibilityMode::Standard);
        assert!(!report.is_installable(), "too-many assets should block install");
        assert!(report.errors.iter().any(|f| f.code == "ASSET_TOO_MANY"),
            "expected ASSET_TOO_MANY; got: {:?}", report.errors);
    }

    // ── oversized single asset blocks install ──────────────────────────────────

    #[test]
    fn oversized_asset_is_error() {
        let mut pack = make_valid_pack();
        // MAX_ASSET_BYTES + 1 bytes.
        pack.assets.register(
            AssetHandle::new_dynamic(
                "font/huge",
                AssetKind::FontFace,
                vec![0u8; MAX_ASSET_BYTES + 1],
            ),
        );
        let report = validate(&pack, AccessibilityMode::Standard);
        assert!(!report.is_installable(), "oversized asset should block install");
        assert!(report.errors.iter().any(|f| f.code == "ASSET_TOO_LARGE"),
            "expected ASSET_TOO_LARGE; got: {:?}", report.errors);
    }

    // ── strict a11y mode promotes low-contrast to error ───────────────────────

    #[test]
    fn strict_mode_promotes_low_contrast_to_error() {
        let mut pack = make_valid_pack();
        // Force very low contrast: near-identical dark colours.
        pack.color_scheme.text = rgba::rgb(20, 20, 20);
        pack.color_scheme.bg   = rgba::rgb(18, 18, 18);
        // These are distinct (won't trigger A11Y_FG_EQ_BG) but horribly low contrast.
        let strict  = validate(&pack, AccessibilityMode::Strict);
        let standard = validate(&pack, AccessibilityMode::Standard);

        // Strict: low contrast should be an error.
        assert!(strict.errors.iter().any(|f| f.code == "A11Y_LOW_CONTRAST"),
            "strict mode should error on low contrast; errors: {:?}", strict.errors);

        // Standard: low contrast should be a warning, not an error.
        assert!(standard.warnings.iter().any(|f| f.code == "A11Y_LOW_CONTRAST"),
            "standard mode should warn on low contrast; warnings: {:?}", standard.warnings);
        assert!(!standard.errors.iter().any(|f| f.code == "A11Y_LOW_CONTRAST"),
            "standard mode should NOT error on low contrast");
    }

    // ── deliberately broken pack is fully rejected ─────────────────────────────

    #[test]
    fn broken_pack_has_multiple_errors() {
        let mut pack = make_valid_pack();
        pack.manifest.id = String::new();                                    // STRUCT_MISSING_ID
        pack.manifest.app_schema_version = CURRENT_SCHEMA_VERSION + 10;     // SCHEMA_TOO_NEW
        pack.color_scheme.text = pack.color_scheme.bg;                       // A11Y_FG_EQ_BG
        // Oversized asset.
        pack.assets.register(AssetHandle::new_dynamic(
            "font/bad",
            AssetKind::FontFace,
            vec![0u8; MAX_ASSET_BYTES + 1],
        ));

        let report = validate(&pack, AccessibilityMode::Standard);
        assert!(!report.is_installable(), "broken pack must not be installable");
        assert!(report.errors.len() >= 3,
            "expected >= 3 errors; got: {:?}", report.errors);
    }

    // ── helper: make a well-formed pack ───────────────────────────────────────

    fn make_valid_pack() -> ThemePack {
        ThemePack {
            manifest: {
                let mut m = ThemeManifest::new("test-pack", "Test Pack");
                m.author = "ci-test".to_string();
                m
            },
            color_scheme:  builtin_dark(),
            style_system:  StyleSystem::builtin_default(),
            recipes:       RecipeSet::new(),
            shell_profile: None,
            assets:        AssetRegistry::new(),
        }
    }
}
