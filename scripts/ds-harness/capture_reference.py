#!/usr/bin/env python3
"""capture_reference.py — screenshot the six ORIGINAL theme apps (ApexTerminalThemes).

The originals are plain HTML files served by the static gallery server:

    cd C:/Users/USER/Documents/development/ApexTerminalThemes
    node server.js          # binds http://localhost:5173

NOTE: this targets the ORIGINALS on :5173, NOT the React port on :5175
(the existing ApexTerminalThemes/terminal/snap_*.py scripts target :5175).

The theme -> page mapping below is transcribed from the THEMES array in
ApexTerminalThemes/server.js (each entry's `folder` + `htmlFiles`).

Output layout (relative to the apex-terminal repo root):

    docs/styling/screenshots/reference/<theme>/<page>-<w>x<h>.png

Usage:
    python capture_reference.py                     # all themes, 1440x900 + 2560x1440
    python capture_reference.py --themes aperture,cadence
    python capture_reference.py --sizes 1440x900
    python capture_reference.py --base-url http://localhost:5173 --out <dir>

Dependency: Playwright for Python + chromium
    pip install playwright && python -m playwright install chromium
"""

import argparse
import asyncio
import sys
import urllib.parse
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]   # .../apex-terminal
DEFAULT_OUT = REPO_ROOT / "docs" / "styling" / "screenshots" / "reference"
DEFAULT_BASE = "http://localhost:5173"
DEFAULT_SIZES = [(1440, 900), (2560, 1440)]

# Transcribed from ApexTerminalThemes/server.js THEMES array.
# theme id -> (folder, [(page_slug, html_path)])
THEMES = {
    "mariner": ("Trading App -  Mariner", [            # note: two spaces in folder name
        ("terminal", "index.html"),
        ("alto-warm-dark", "Alto Warm Dark.html"),
    ]),
    "alto": ("Trading App - alto", [
        ("terminal", "Alto Warm Dark.html"),
        ("the-daily", "The Daily.html"),
        ("daily", "daily.html"),
    ]),
    "aperture": ("Trading App - Aperture", [
        ("trading-terminal", "Trading Terminal.html"),
    ]),
    "cadence": ("Trading App - Cadence", [
        ("trading-app", "Trading App.html"),
    ]),
    "lucid": ("trading app - Lucid _ new _", [
        ("terminal", "index.html"),
    ]),
    "meridien": ("trading app - meridien", [
        ("terminal", "index.html"),
    ]),
}


def server_alive(base_url: str) -> bool:
    try:
        with urllib.request.urlopen(base_url + "/api/themes", timeout=3) as r:
            return r.status == 200
    except Exception:
        return False


async def run(base_url: str, out_dir: Path, themes: list[str], sizes: list[tuple[int, int]]):
    try:
        from playwright.async_api import async_playwright
    except ImportError:
        print("ERROR: Playwright for Python is not installed.", file=sys.stderr)
        print("  pip install playwright && python -m playwright install chromium", file=sys.stderr)
        sys.exit(2)

    written = []
    async with async_playwright() as p:
        # Same GPU-safe flags the existing ApexTerminalThemes snapshot scripts use.
        browser = await p.chromium.launch(
            headless=True,
            args=["--enable-unsafe-swiftshader", "--use-gl=swiftshader",
                  "--ignore-gpu-blocklist"],
        )
        for w, h in sizes:
            ctx = await browser.new_context(viewport={"width": w, "height": h})
            page = await ctx.new_page()
            for theme in themes:
                folder, pages = THEMES[theme]
                for slug, html_path in pages:
                    # Folder / file names contain spaces -> percent-encode.
                    url = f"{base_url}/{urllib.parse.quote(folder)}/{urllib.parse.quote(html_path)}"
                    dest = out_dir / theme / f"{slug}-{w}x{h}.png"
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    try:
                        await page.goto(url, wait_until="networkidle", timeout=30_000)
                    except Exception as e:
                        print(f"  !! {theme}/{slug}: goto failed ({e}); trying 'load'")
                        await page.goto(url, wait_until="load", timeout=30_000)
                    # Let fonts/CSS animations settle (originals animate sparklines etc.)
                    await page.wait_for_timeout(1200)
                    await page.screenshot(path=str(dest), full_page=False)
                    size = dest.stat().st_size
                    written.append(dest)
                    print(f"  -> {dest.relative_to(REPO_ROOT)}  ({size:,} bytes)")
            await ctx.close()
        await browser.close()

    print(f"\nDone: {len(written)} screenshot(s) under {out_dir}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--base-url", default=DEFAULT_BASE)
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--themes", default=",".join(THEMES),
                    help="comma-separated subset of: " + ",".join(THEMES))
    ap.add_argument("--sizes", default=",".join(f"{w}x{h}" for w, h in DEFAULT_SIZES),
                    help='comma-separated WxH list, e.g. "1440x900,2560x1440"')
    args = ap.parse_args()

    themes = [t.strip() for t in args.themes.split(",") if t.strip()]
    unknown = [t for t in themes if t not in THEMES]
    if unknown:
        ap.error(f"unknown theme(s): {unknown}; valid: {list(THEMES)}")
    sizes = []
    for s in args.sizes.split(","):
        w, _, h = s.strip().partition("x")
        sizes.append((int(w), int(h)))

    if not server_alive(args.base_url):
        print(f"ERROR: originals server not reachable at {args.base_url}", file=sys.stderr)
        print("Start it with:  cd <ApexTerminalThemes dir> && node server.js", file=sys.stderr)
        sys.exit(1)

    asyncio.run(run(args.base_url, args.out, themes, sizes))


if __name__ == "__main__":
    main()
