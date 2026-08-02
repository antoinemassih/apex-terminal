#!/usr/bin/env python3
"""pixel_assert.py — assert sampled pixel values in a PNG against a JSON spec.

Spec format (JSON):
{
  "_comment": "free-text keys starting with _ are ignored",
  "image": "optional/default/image/path.png",
  "samples": [
    { "name": "bg",    "x": 10,  "y": 500, "expected_rgb": "#000000", "tolerance": 4 },
    { "name": "panel", "x": 120, "y": 80,  "expected_rgb": [20, 19, 17], "tolerance": 4 }
  ]
}

- expected_rgb: "#rrggbb" string or [r, g, b] list.
- tolerance: max allowed per-channel absolute difference (default 4).
- The image is taken from --image if given, else the spec's "image" field.

Usage:
    python pixel_assert.py --spec ramps.aperture.json --image <capture.png>
    python pixel_assert.py --spec ramps.aperture.json          # uses spec's "image"

Exit code 0 if all samples pass, 1 otherwise. Requires Pillow (pip install pillow).
"""

import argparse
import json
import sys
from pathlib import Path


def parse_rgb(v):
    if isinstance(v, str):
        s = v.lstrip("#")
        if len(s) != 6:
            raise ValueError(f"bad hex color: {v}")
        return tuple(int(s[i:i + 2], 16) for i in (0, 2, 4))
    if isinstance(v, (list, tuple)) and len(v) >= 3:
        return tuple(int(c) for c in v[:3])
    raise ValueError(f"bad expected_rgb: {v!r}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--spec", type=Path, required=True, help="JSON spec file")
    ap.add_argument("--image", type=Path, default=None,
                    help="PNG to sample (overrides spec's 'image' field)")
    args = ap.parse_args()

    try:
        from PIL import Image
    except ImportError:
        print("ERROR: Pillow is required (pip install pillow)", file=sys.stderr)
        sys.exit(2)

    spec = json.loads(args.spec.read_text(encoding="utf-8"))
    img_path = args.image or (Path(spec["image"]) if spec.get("image") else None)
    if img_path is None:
        print("ERROR: no image given (--image or spec 'image' field)", file=sys.stderr)
        sys.exit(2)
    if not img_path.is_file():
        print(f"ERROR: image not found: {img_path}", file=sys.stderr)
        sys.exit(2)

    img = Image.open(img_path).convert("RGB")
    w, h = img.size

    samples = [s for s in spec.get("samples", []) if isinstance(s, dict)]
    if not samples:
        print("ERROR: spec has no samples", file=sys.stderr)
        sys.exit(2)

    rows = []
    failed = 0
    for s in samples:
        name = s.get("name", "?")
        x, y = int(s["x"]), int(s["y"])
        tol = int(s.get("tolerance", 4))
        exp = parse_rgb(s["expected_rgb"])
        if not (0 <= x < w and 0 <= y < h):
            rows.append((name, x, y, exp, None, tol, "OUT-OF-BOUNDS"))
            failed += 1
            continue
        got = img.getpixel((x, y))[:3]
        delta = max(abs(a - b) for a, b in zip(exp, got))
        ok = delta <= tol
        if not ok:
            failed += 1
        rows.append((name, x, y, exp, got, tol, f"PASS (d={delta})" if ok else f"FAIL (d={delta})"))

    def hexs(rgb):
        return "#%02x%02x%02x" % rgb if rgb else "-"

    print(f"image: {img_path}  ({w}x{h})")
    print(f"{'sample':<14} {'x':>5} {'y':>5} {'expected':<9} {'got':<9} {'tol':>3}  result")
    print("-" * 62)
    for name, x, y, exp, got, tol, result in rows:
        print(f"{name:<14} {x:>5} {y:>5} {hexs(exp):<9} {hexs(got):<9} {tol:>3}  {result}")
    print("-" * 62)
    print(f"{len(rows) - failed}/{len(rows)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
