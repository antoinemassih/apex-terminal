#!/usr/bin/env python3
"""contact_sheet.py — side-by-side reference | current grid per theme.

Scans:
    docs/styling/screenshots/reference/<theme>/<page>-<w>x<h>.png
    docs/styling/screenshots/current/<theme>-<style>-<w>x<h>.png

and writes docs/styling/screenshots/contact_sheet.html — plain HTML with
inline CSS and RELATIVE image paths, so opening it via file:// just works.

Usage:
    python contact_sheet.py
    python contact_sheet.py --root <screenshots dir> --out <html path>
"""

import argparse
import html
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_ROOT = REPO_ROOT / "docs" / "styling" / "screenshots"

THEME_ORDER = ["aperture", "cadence", "alto", "mariner", "lucid", "meridien"]

CSS = """
  body { margin: 0; padding: 24px; background: #111; color: #ddd;
         font: 13px/1.5 -apple-system, "Segoe UI", sans-serif; }
  h1 { font-size: 18px; font-weight: 600; margin: 0 0 16px; }
  h2 { font-size: 15px; font-weight: 600; margin: 32px 0 8px;
       text-transform: capitalize; border-bottom: 1px solid #333;
       padding-bottom: 4px; }
  .row { display: flex; gap: 16px; flex-wrap: wrap; margin-bottom: 16px; }
  .cell { flex: 1 1 460px; max-width: 720px; }
  .cell .tag { font-size: 11px; color: #999; margin: 0 0 4px;
               text-transform: uppercase; letter-spacing: 0.05em; }
  .cell .tag b { color: #ccc; }
  .cell img { width: 100%; height: auto; display: block;
              border: 1px solid #333; border-radius: 4px; background: #000; }
  .missing { display: flex; align-items: center; justify-content: center;
             min-height: 200px; border: 1px dashed #444; border-radius: 4px;
             color: #666; }
  .note { color: #888; font-size: 12px; }
"""


def cell(title: str, img: Path | None, root: Path) -> str:
    if img is None:
        return (f'<div class="cell"><p class="tag">{html.escape(title)}</p>'
                f'<div class="missing">no capture</div></div>')
    rel = img.relative_to(root).as_posix()
    return (f'<div class="cell"><p class="tag"><b>{html.escape(title)}</b> '
            f'&middot; {html.escape(img.name)}</p>'
            f'<a href="{rel}"><img src="{rel}" loading="lazy"></a></div>')


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--root", type=Path, default=DEFAULT_ROOT,
                    help="screenshots dir containing reference/ and current/")
    ap.add_argument("--out", type=Path, default=None,
                    help="output html (default <root>/contact_sheet.html)")
    args = ap.parse_args()
    root: Path = args.root
    out: Path = args.out or (root / "contact_sheet.html")

    ref_dir = root / "reference"
    cur_dir = root / "current"

    # reference: reference/<theme>/*.png
    ref_by_theme: dict[str, list[Path]] = {}
    if ref_dir.is_dir():
        for d in sorted(ref_dir.iterdir()):
            if d.is_dir():
                ref_by_theme[d.name] = sorted(d.glob("*.png"))

    # current: current/<theme>-<style>-<w>x<h>.png  (theme = first '-' token)
    cur_by_theme: dict[str, list[Path]] = {}
    if cur_dir.is_dir():
        for f in sorted(cur_dir.glob("*.png")):
            theme = f.name.split("-", 1)[0]
            cur_by_theme.setdefault(theme, []).append(f)

    themes = [t for t in THEME_ORDER if t in ref_by_theme or t in cur_by_theme]
    themes += sorted((set(ref_by_theme) | set(cur_by_theme)) - set(themes))

    sections = []
    for theme in themes:
        refs = ref_by_theme.get(theme, [])
        curs = cur_by_theme.get(theme, [])
        rows = []
        n = max(len(refs), len(curs), 1)
        for i in range(n):
            r = refs[i] if i < len(refs) else None
            c = curs[i] if i < len(curs) else None
            rows.append('<div class="row">'
                        + cell("reference", r, root)
                        + cell("current (apex-terminal)", c, root)
                        + "</div>")
        sections.append(f"<h2>{html.escape(theme)}</h2>\n" + "\n".join(rows))

    doc = f"""<!DOCTYPE html>
<html><head><meta charset="utf-8">
<title>apex-terminal DS contact sheet</title>
<style>{CSS}</style></head>
<body>
<h1>Design-system contact sheet &mdash; reference vs current</h1>
<p class="note">reference = original theme HTML (ApexTerminalThemes, :5173) &middot;
current = live apex-terminal capture via dev_inspector. Regenerate with
<code>python scripts/ds-harness/contact_sheet.py</code>.</p>
{"".join(sections) if sections else "<p class='note'>No screenshots found — run the capture scripts first.</p>"}
</body></html>
"""
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(doc, encoding="utf-8")
    print(f"Wrote {out}  ({len(themes)} theme section(s))")


if __name__ == "__main__":
    main()
