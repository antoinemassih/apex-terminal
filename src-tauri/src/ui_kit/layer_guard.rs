//! Layer-boundary guard for `ui_kit`.
//!
//! `ui_kit` is the canonical, self-contained component layer: everything else
//! depends on it, and it depends on nothing above it. The two legacy component
//! systems (`chart/renderer/ui/components/` and `chart/renderer/ui/panels/kit.rs`)
//! cannot be retired while `ui_kit` still reaches back into them.
//!
//! This module is tests only — it scans the `ui_kit` source tree and fails if a
//! `chart_renderer::` / `chart::renderer::` reference appears in code (comments
//! and doc-comments are exempt; they document the boundary rather than cross it).
//!
//! ### The allowlist is a RATCHET, not a permission slip
//! [`KNOWN_VIOLATIONS`] pins the exact remaining count per file. Adding a
//! reference anywhere — including to an allowlisted file — fails the test.
//! Removing one fails it too, with a message telling you to lower the number.
//! The only legal direction is down, to zero.

#![cfg(test)]

use std::path::{Path, PathBuf};

/// Files that still reach into `chart_renderer`, with the EXACT number of
/// code (non-comment) references each is allowed to contain.
///
/// * `widgets/frames.rs` — `PanelFrame` / `CardFrame` / `PopupFrame` /
///   `CompactPanelFrame` read the chart `StyleSettings` through
///   `chart_renderer::ui::style::current()` (`hairline_borders`,
///   `shadows_enabled`, `shadow_blur`, `shadow_offset_y`, `card_padding_*`),
///   none of which exist on `ui_kit`'s `TokenSnapshot`; and
///   `PanelHeaderWithClose` delegates to `panels::kit::PanelHeader`, which is
///   hard-coupled to `gpu::Watchlist` pane-header metrics. Neither is
///   expressible through `ComponentTheme`, so neither can be inverted without
///   first porting `StyleSettings` and `Watchlist` — out of scope here.
/// * `tokens.rs` — a back-compat glob re-export
///   (`pub use chart_renderer::ui::style::*`) kept for ~40 chart-app callers
///   that import chart-specific style helpers through this bridge. Severing it
///   is a per-callsite import sweep, not a dependency inversion.
const KNOWN_VIOLATIONS: &[(&str, usize)] = &[
    ("widgets/frames.rs", 3),
    ("tokens.rs", 1),
];

/// Strip `//`-line and `/* */`-block comments so only real code is scanned.
/// Deliberately simple: `ui_kit` has no string literal containing `//`, and
/// erring toward stripping can only make the guard *more* lenient on a line
/// it misreads — never falsely accuse.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_block = false;
    for line in src.lines() {
        let b = line.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if in_block {
                if i + 1 < b.len() && b[i] == b'*' && b[i + 1] == b'/' {
                    in_block = false;
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'/' {
                break; // rest of line is a comment
            }
            if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'*' {
                in_block = true;
                i += 2;
                continue;
            }
            out.push(b[i] as char);
            i += 1;
        }
        out.push('\n');
    }
    out
}

fn ui_kit_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("ui_kit")
}

/// All `.rs` files under `src/ui_kit/`, as (relative-path, contents).
fn ui_kit_sources() -> Vec<(String, String)> {
    let root = ui_kit_root();
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = path
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let src = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                out.push((rel, src));
            }
        }
    }
    out.sort();
    out
}

/// Count code-level references to the chart layer in one file's source.
fn violation_count(src: &str) -> usize {
    let code = strip_comments(src);
    code.matches("chart_renderer::").count() + code.matches("chart::renderer::").count()
}

/// Files allowed to import the `tokens::*` glob bridge.
///
/// AUDIT 2026-08-02 (AT-060, W2): the main guard below counts literal
/// `chart_renderer::` references per file. `tokens.rs` is pinned at 1 for its
/// `pub use crate::chart_renderer::ui::style::*` re-export — but that single
/// pinned reference is a GLOB. Any ui_kit file can write `use ...tokens::*`
/// and pull chart-layer items in transitively while containing zero
/// `chart_renderer::` text of its own, so the main guard scores it clean.
///
/// That is the hole: the boundary was enforced on spelling, not on reachability.
///
/// Severing the glob outright is a per-callsite import sweep across ~40 chart
/// callers — out of scope here. Pinning the consumers is not: it cannot widen
/// without this list changing, which makes the next one a deliberate decision
/// instead of an accident.
const GLOB_BRIDGE_CONSUMERS: &[&str] = &[
    "widgets/context_menu.rs",
    "widgets/shell_variants.rs",
];

/// AUDIT 2026-08-02 (AT-060, W2): pin the glob-bridge consumer set.
#[test]
fn ui_kit_glob_bridge_consumers_do_not_grow() {
    let files = ui_kit_sources();
    let mut found: Vec<String> = Vec::new();
    for (rel, src) in &files {
        if rel == "layer_guard.rs" || rel == "tokens.rs" {
            continue;
        }
        if strip_comments(src).contains("tokens::*") {
            found.push(rel.clone());
        }
    }
    found.sort();
    let mut expected: Vec<String> =
        GLOB_BRIDGE_CONSUMERS.iter().map(|s| (*s).to_string()).collect();
    expected.sort();

    assert_eq!(
        found, expected,
        "\nThe `tokens::*` glob bridge re-exports `chart_renderer::ui::style::*`, so \
         importing it pulls the chart layer into ui_kit WITHOUT any literal \
         `chart_renderer::` reference for the main layer guard to catch.\n\n\
         If you added a file here: import the specific items you need from \
         `ui_kit::style` instead, or — if the item only exists on the chart side — \
         it does not belong in ui_kit.\n\n\
         If you REMOVED one: delete it from GLOB_BRIDGE_CONSUMERS to lock the gain in.\n"
    );
}

/// THE layer-boundary test.
///
/// Every file under `src/ui_kit/` must be free of `chart_renderer::` /
/// `chart::renderer::` in code, except the pinned entries in
/// [`KNOWN_VIOLATIONS`], whose counts must match exactly.
#[test]
fn ui_kit_does_not_depend_on_chart_renderer() {
    let files = ui_kit_sources();
    assert!(
        files.len() > 30,
        "source walk found only {} files under src/ui_kit — the walk is broken, \
         not the layer boundary",
        files.len()
    );

    let mut new_violations: Vec<String> = Vec::new();
    let mut stale_pins: Vec<String> = Vec::new();

    for (rel, src) in &files {
        // `layer_guard.rs` names both paths as data (KNOWN_VIOLATIONS docs +
        // the match patterns themselves); it is the guard, not a consumer.
        if rel == "layer_guard.rs" {
            continue;
        }
        let n = violation_count(src);
        let allowed = KNOWN_VIOLATIONS
            .iter()
            .find(|(f, _)| *f == rel)
            .map(|(_, n)| *n)
            .unwrap_or(0);

        if n > allowed {
            new_violations.push(format!(
                "  src/ui_kit/{rel}: {n} chart-layer reference(s), {allowed} allowed"
            ));
        } else if n < allowed {
            stale_pins.push(format!(
                "  src/ui_kit/{rel}: down to {n} (pinned at {allowed}) — lower the \
                 KNOWN_VIOLATIONS entry, or delete it if {n} == 0"
            ));
        }
    }

    assert!(
        new_violations.is_empty(),
        "ui_kit must not depend on the chart layer — the dependency runs \
         chart_renderer -> ui_kit, never the reverse.\n{}\n\n\
         Fix: move what ui_kit needs INTO ui_kit and make the chart type a \
         delegating wrapper (see ui_kit::widgets::DialogHeader / \
         chart_renderer::ui::components::headers_widget::DialogHeaderWithClose \
         for the pattern). If the piece needs the full chart `Theme`, express \
         it through the `ComponentTheme` trait instead.",
        new_violations.join("\n")
    );

    assert!(
        stale_pins.is_empty(),
        "the KNOWN_VIOLATIONS ratchet is stale — progress was made but not \
         recorded, so it can silently regress:\n{}",
        stale_pins.join("\n")
    );

    // Every pinned file must still exist, or the pin is dead weight that would
    // mask a violation if the path came back.
    for (f, _) in KNOWN_VIOLATIONS {
        assert!(
            files.iter().any(|(rel, _)| rel == f),
            "KNOWN_VIOLATIONS names src/ui_kit/{f}, which no longer exists — \
             remove the entry"
        );
    }
}

/// The specific inversion this guard was written for: the dialog header is
/// OWNED by ui_kit, and ui_kit's Modal consumes the ui_kit type — not the
/// legacy `chart_renderer::…::DialogHeaderWithClose`.
///
/// `include_str!` (rather than the fs walk) so the check is compiled against
/// the exact sources, matching the existing source-scanning test style in
/// `panel_list_row.rs::source_guards_ease_bool_with_hoverable`.
#[test]
fn dialog_header_is_owned_by_ui_kit() {
    let dialog_header = include_str!("widgets/dialog_header.rs");
    assert!(
        dialog_header.contains("pub struct DialogHeader<'a>"),
        "ui_kit must OWN DialogHeader, not re-export it from the chart layer"
    );
    assert!(
        !strip_comments(dialog_header).contains("chart_renderer"),
        "ui_kit::widgets::DialogHeader must not reach into chart_renderer"
    );

    let modal = strip_comments(include_str!("widgets/modal.rs"));
    assert!(
        !modal.contains("DialogHeaderWithClose"),
        "Modal must use ui_kit's DialogHeader, not the legacy \
         chart_renderer DialogHeaderWithClose"
    );
    assert!(
        modal.contains("DialogHeader::new(title)"),
        "Modal's Dialog header style must render through ui_kit's DialogHeader"
    );
}

/// The widget-level bug-report instrumentation is ui_kit-owned too: widgets
/// call `ui_kit::inspect`, and the chart layer re-exports it. Before the
/// inversion ~10 widgets called `chart_renderer::bug_anchor::…` directly.
#[test]
fn widget_inspect_anchors_are_owned_by_ui_kit() {
    for (rel, src) in ui_kit_sources() {
        if rel == "layer_guard.rs" {
            continue;
        }
        assert!(
            !strip_comments(&src).contains("bug_anchor::"),
            "src/ui_kit/{rel} calls chart_renderer's bug_anchor — use \
             crate::ui_kit::inspect instead"
        );
    }
}

#[test]
fn strip_comments_removes_line_and_block_comments() {
    assert_eq!(strip_comments("//! chart_renderer::x\n").trim(), "");
    assert_eq!(strip_comments("// chart_renderer::x\n").trim(), "");
    assert_eq!(strip_comments("let a = 1; // chart_renderer::x\n").trim(), "let a = 1;");
    assert_eq!(strip_comments("/* chart_renderer::x */\n").trim(), "");
    assert_eq!(
        strip_comments("/* chart_renderer::x\n   more */ let b = 2;\n").trim(),
        "let b = 2;"
    );
    // Code survives untouched.
    assert!(strip_comments("use crate::chart_renderer::gpu;\n").contains("chart_renderer::"));
}
