# Visual audit capture set — 2026-08-05

Twelve distinct app surfaces captured through the dev_inspector harness for a
visual (not structural) design audit.

## How these were made

Driven by `POST /run-scenario` against a live app, one surface per shot:

```
SetWatchlistTab 1|2|3 · SetScannerOpen · SetRrgOpen · SetOrderPanel
SetDomSidebar · SetObjectTree · SetPlaybookPanel · SetAutoChartPanel
click_widget toolbar.settings_btn
```

Theme/style pair is whatever the app was on (Aperture-family dark). Theme
FIDELITY is audited separately against `../screenshots/current/`, which holds
one shot per certified design system.

## Reading these

- **3840x2088, and the window spans TWO 1920 monitors.** There is a seam
  artifact at x≈1920: thin vertical lines, black blocks top and bottom, and a
  stray glyph near the bottom edge. It is NOT an app defect. It reads
  convincingly like a divider painted through the "Indicators" label — do not
  chase it.
- `07-orders-panel.png` is byte-identical to `05-scanner.png`: the
  `OpenOrdersPanel` command did not take effect. It is a duplicate, not a
  screen.
- Empty panels, zeroed P&L and "not connected" are app STATE, not design
  defects. How an empty state is PRESENTED is still fair game.

## Files

| File | Surface |
|---|---|
| 01-default | Chart + watchlist LIST, nothing else open |
| 02/03/04-watchlist-* | Watchlist CHAIN / HEAT / SCAN tabs |
| 05-scanner | Scanner panel |
| 06-rrg | Relative rotation graph |
| 07-orders-panel | (duplicate of 05 — command no-op) |
| 08-order-entry | Order entry form |
| 09-dom-sidebar | DOM ladder sidebar |
| 10-object-tree | Drawing/object tree |
| 11-playbook | Playbook panel |
| 12-auto-chart | Auto-chart panel |
| 13-settings | Settings modal, Appearance tab |
