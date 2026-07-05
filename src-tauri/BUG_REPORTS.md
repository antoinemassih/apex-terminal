# Bug Reports

Anchored bug reports captured from the running app (Ctrl+Shift+I Bug Inspect mode).
Each report header: `## [ ] @anchor <key>  (<file>:<line>)`. Check the box when fixed.

## [x] @anchor pane/0/QQQ  (src\chart\renderer\render\pane\core.rs:152)
_2026-06-28 06:07 UTC_  ·  bug-0001

the chart is showing bars with extremely long wicks as shown in the screenshot thats not what hte data looks like on other charting software

**FIXED 2026-06-28** — The live-bar handler treated ApexData's full-minute
aggregate with sticky `min`/`max` (`l.low = l.low.min(bar.low)`), so a single
transient bad frame permanently corrupted a building bar's low and rendered an
extreme wick that never recovered even after good frames arrived (the long-wick
"comb"). Fix in `gpu.rs` UpdateLastBar cumulative branch: trust the server's
aggregate (replace high/low instead of accumulating), ignore non-positive
prices, and keep OHLC self-consistent (wick brackets the body). Historical/REST
bars were already correct; this only affected the live feed.

_(original region + comparison screenshots were inadvertently cleared during
testing — apologies; the fix is verified by build + code review)_

---

## [x] @anchor pane/1/QQQ  (src\chart\renderer\render\pane\core.rs:152)
_2026-06-28 06:08 UTC_  ·  bug-0002

when i open a chart pane it loads the data of a stock endlessly i have to select another stock or reselect the ticker for it to load

**FIXED 2026-06-28** — A pane created by splitting was built with `Chart::new()`
but never set `pending_symbol_change`, and the bar fetch only fires when that
field is taken — so the new pane sat on the loading spinner forever until you
reselected the ticker (which set the field). Fix in `core.rs` pane-split path:
build the new pane with `Chart::new_with(symbol, timeframe)` inheriting the
source pane's symbol/timeframe, and set `pending_symbol_change` so the initial
bar fetch fires immediately.

---
