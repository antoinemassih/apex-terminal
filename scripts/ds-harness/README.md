# ds-harness — design-system verification loop (ticket M0.1)

Screenshot + pixel-assert harness for comparing the live apex-terminal app
against the six original theme mockups in `ApexTerminalThemes`.

All output lands under `docs/styling/screenshots/`:

```
docs/styling/screenshots/
├── reference/<theme>/<page>-<w>x<h>.png     # originals (HTML mockups)
├── current/<theme>-<style>-<w>x<h>.png      # live app captures
└── contact_sheet.html                       # side-by-side grid
```

## Dependencies

- Python 3 with **Playwright** (`pip install playwright && python -m playwright install chromium`)
  — needed by `capture_reference.py` only.
- **Pillow** (`pip install pillow`) — needed by `pixel_assert.py` only.
- `capture_app.py` and `contact_sheet.py` are stdlib-only.
- Node (any recent) to run the originals server.

## The full loop

### 1. Start the originals server

```
cd C:/Users/USER/Documents/development/ApexTerminalThemes
node server.js        # static gallery server on http://localhost:5173
```

### 2. Capture reference screenshots

```
python scripts/ds-harness/capture_reference.py
# subset / custom sizes:
python scripts/ds-harness/capture_reference.py --themes aperture,cadence --sizes 1440x900
```

Captures each theme's main page(s) (from the `THEMES` array in
ApexTerminalThemes/server.js) at 1440x900 and 2560x1440. Kill the node server
afterwards (kill the specific PID — never `taskkill node.exe`).

### 3. Orchestrator builds + launches the app

**Not this harness's job.** The app must be a **debug** build (the
dev_inspector HTTP harness is `#[cfg(debug_assertions)]`-only) with a
**visible** window (screenshots BitBlt the real HWND — not `--headless`).
Default port 7892, overridable via `APEX_DEV_INSPECTOR_PORT`.

### 4. Capture the live app

```
python scripts/ds-harness/capture_app.py --probe    # sanity: /health + /state
python scripts/ds-harness/capture_app.py            # default DS sweep
python scripts/ds-harness/capture_app.py --pairs aperture:aperture,lucid:lucid
python scripts/ds-harness/capture_app.py --pairs 16:1,20:6 --size 1440x900
```

Per pair it POSTs `/cmd {"cmd":"SetThemeIdx","idx":T}` +
`{"cmd":"SetStyleIdx","idx":S}`, then `/screenshot {"name":...}`; the app
writes `dev/screenshots/<name>.png` (repo-root relative) and the script copies
it to `docs/styling/screenshots/current/<theme>-<style>-<w>x<h>.png`.
Theme/style index↔name maps are documented in `capture_app.py`'s header
(source: `src-tauri/src/design_system/builtin.rs`).

### 5. Build the contact sheet

```
python scripts/ds-harness/contact_sheet.py
# then open docs/styling/screenshots/contact_sheet.html (file:// works)
```

### 6. Pixel asserts

```
python scripts/ds-harness/pixel_assert.py --spec scripts/ds-harness/ramps.aperture.json \
    --image docs/styling/screenshots/current/aperture-aperture-<w>x<h>.png
```

`ramps.aperture.json` asserts the authored Aperture surface ramp
(bg `#000000`, panel `#141311`, surface `#1a1816`, elevated `#1f1d1a` — from
`docs/DESIGN_BRIEF_DS_ADOPTION.md` §5.1). **Its x/y coordinates are
placeholders** — fill them in after the first live capture by picking pixels
squarely inside each surface. Exit code is non-zero on any failure, so it can
gate CI/orchestrator steps.

## Gotchas

- dev_inspector rejects any request carrying an `Origin` header (CSRF guard);
  the scripts use urllib and send none.
- `POST /screenshot` does **not** return PNG bytes — it returns
  `{"ok":true,"path":"dev/screenshots/<name>.png"}` after the render thread
  writes the file. The scripts poll for the file to dodge the race.
- Screenshot size = live window size. Use `--size WxH`
  (`SetViewportSize`) to normalize before a sweep.
- Zombie apex-native.exe processes lock the exe **and** may still hold port
  7892 (the server retries bind every 2 s) — if `/health` answers but the
  window is gone, you're talking to a zombie.
