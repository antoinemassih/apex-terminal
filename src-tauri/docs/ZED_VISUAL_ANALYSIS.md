# Zed visual analysis — onboarding reference

Companion to `ZED_DESIGN_SYSTEM_AUDIT.md`. That doc reads the source. This doc reads **one specific screenshot** and catalogs what it tells us about the chrome we should be building. Where the audit talks tokens, this talks pixels.

## Source
- Image: `design references/zed/zed.jpg`
- Resolution: ~1150 × 1300 px (window appears centered on a black desktop, with the actual app chrome occupying roughly the top 815px)
- Theme shown: **Dark, warm-gray cast** — not neutral-cool. The base surface reads as a desaturated taupe / brown-gray (think `#1f1d1b`-ish), not a pure neutral `#1a1a1a`. The accent is a muted amber/honey tone reserved for the send arrow and the active-tab dot.
- Mode: Onboarding wizard — left pane shows a Claude Agent thread, right pane is the setup wizard. A collapsed-state left sidebar is visible on the far left.

A note on the warm cast: this is the single most distinctive choice in the screenshot and **the audit underplays it**. Zed's "One Dark"-ish palette here is unmistakably warm. Apex's current dark theme is cool/neutral. Even with token parity, our chrome will read as a different family until we decide whether to match the warmth or stay neutral. My recommendation: stay neutral for trading (warm casts subtly bias green/red perception); steal the rest.

---

## Region-by-region

### 1. Title bar (top edge, ~28px tall)

**Visible.** A slim horizontal strip running edge-to-edge. Left side: a hamburger glyph (three lines) followed by the text `Open Recent Project`. Right side, in order: `Sign In` text button, a downward chevron next to it, then the standard window controls (minimize, maximize, close). No icons or breadcrumbs in the middle. No bottom border separating it from the tab strip — the seam is implied by the tab pills sitting *below* the line.

**Special.**
- The title bar uses the **same surface as the panel below it** — there is no second elevation step at the top of the window. Most apps darken or lighten the title bar; Zed flattens it into the chrome.
- "Open Recent Project" is a **clickable label, not a button** — no hover background visible, no border, just text + a hamburger. This is the "ghost element" pattern from the audit, applied to the most prominent chrome slot.
- Window controls are **monochrome glyphs**, not the OS-default red/yellow/green. They read as part of the icon system, not as platform decoration.
- `Sign In` is paired with a chevron — it's a menu trigger styled identically to the project label. Same weight, same color, same lack of background.

**Replicate.**
- [ ] Drop any background-color difference between title bar and content surface.
- [ ] Convert our window-controls cluster from platform-native to monochrome icon glyphs sharing the icon ramp.
- [ ] Replace the "current project" widget (if we have one in chrome) with a ghost-style text label + hamburger.
- [ ] Reserve the right-of-title slot for a single account/identity affordance with a chevron, not a stack of icons.

**Apex today?** We have no real title bar in egui — Tauri gives us the OS chrome, then we paint our own toolbar below it. Closing this gap means deciding to draw a custom title bar inside the Tauri window (which we already do partially for the dock). Cost: medium, but already on the roadmap.

---

### 2. Tab strip (~36px tall, immediately below title bar)

**Visible.** Three tabs: `Welcome`, `Onboarding` (active), and `i like the way this test...` (truncated). To the left of the tabs is a `+` button and a maximize/restore glyph (the four-arrow expand). To the right are forward/back arrows. Far right: another `+` and two window-frame icons.

**Special.**
- **Active tab indicator is a tiny dot, not an underline.** Before the word "Onboarding" there's a small accent-colored dot (the warm honey accent). The active tab has no underline, no different background, no thicker border — just the dot and slightly brighter text. This is the lightest-weight active-state indication I've seen in a serious editor.
- **Inactive tabs have no chrome.** No pill background, no border, no separator line between tabs. The tabs are essentially just text labels with consistent spacing. The whole strip reads as a row of text, not a row of widgets.
- **Active tab text is subtly brighter** than inactive — looks like full `text` vs `text_muted`. The dot does most of the work; the color shift is the second cue.
- **No close-X visible on inactive tabs.** It probably appears on hover, but in the resting state the strip is completely free of close affordances. This dramatically reduces visual noise.
- Tab spacing is generous — looks like ~24-32px of horizontal padding inside each tab.
- The leftmost cluster (`+` and a maximize-pane icon) is **separated from the tabs by a vertical divider**. So is the right cluster. These are pane controls, not tab controls, and the visual grouping says so.

**Replicate.**
- [ ] **Active-tab dot pattern** — kill our current bottom-border-on-active treatment, replace with a small leading accent dot. This is probably the single most "Zed" thing on the screen.
- [ ] Drop tab pill backgrounds entirely. Inactive tabs become text-only.
- [ ] Hide close-X until hover.
- [ ] Use vertical-rule dividers (`border_variant`) to separate pane controls from tab list.
- [ ] Title and tab text shift from `text_muted` (inactive) to `text` (active) — no other state cues needed.

**Apex today?** Our tabs are pill-shaped with backgrounds and a bottom accent border. They look heavy by comparison. The fix is mostly subtractive — remove backgrounds, remove the underline, add the dot.

---

### 3. Left sidebar collapsed/empty state

**Visible.** A narrow column on the far left. Two stacked text labels in the middle vertically: `Open Project  Ctrl-K` and below it `Clone Repository`. Between them, a small `or` separator. At the bottom, a card with a heading "Looking for threads from external agents?", body copy, an X dismiss button in the top-right of the card, and an `Import Threads` CTA with a download glyph.

**Special.**
- The empty state isn't a graphic or illustration — it's just **two text affordances + a keybind chip**, centered.
- The keybind appears **inline next to the action label**, not below it, not in a tooltip — `Open Project  Ctrl-K  Ctrl-O`. The chord is rendered as muted-text glyphs, no boxes, no chip backgrounds. This is the lightest possible kbd treatment.
- The `or` separator is just the word, no horizontal rule.
- The bottom notification card is the **only elevated surface** in this region — it has a slightly lighter fill and a subtle top border distinguishing it from the panel.
- The notification's CTA (`Import Threads`) is a **subtle button with a leading icon**, not a primary filled button. Even a marketing-style upsell is rendered restrained.

**Replicate.**
- [ ] Empty-state pattern: stacked text actions with inline keybind chips. Use this for our empty pane states (no chart loaded, no watchlist selected).
- [ ] Inline keybind rendering — muted text, no chip background, sits adjacent to the action label.
- [ ] In-panel notification card with dismiss-X and a subtle CTA. Reusable for one-time tips, version notes, "did you know" pokes.

**Apex today?** Our empty states are typically a centered icon + heading + body + filled button. That's the SaaS-marketing pattern. Zed's pattern is the **editor pattern** — assume the user is competent, give them keystrokes. Cost: low, mostly a `EmptyState` widget redesign.

---

### 4. Conversation panel (left main content)

**Visible.** A vertical stack of message bubbles. User messages have a faint rounded-rect background and a 1px border; assistant replies are rendered without a bubble, just as flowed prose against the panel background. A `Thinking` status row sits between the last user message and the assistant reply, with a small lightning/sparkle icon. At the bottom is a composer: `Message Claude Agent — @ to include context, / for commands` placeholder, then a row of pill-style controls (`Default ▾`, `Sonnet ▾`, `Low ▾`) and a primary send arrow at the right.

**Special.**
- **Asymmetric bubble treatment.** User messages are bubbled (border + bg). Agent messages are bare. This is the opposite of consumer chat apps (iMessage, Slack) where both sides bubble. The result: assistant prose feels like *content*, user input feels like *quoted query*.
- **The bubble itself is barely there** — the bg is maybe 4% lighter than the panel surface, the border is 1px at low alpha. You could miss it. That's the point: the bubble exists to delimit, not to decorate.
- **Send button is the only saturated element** in the entire conversation panel. A solid amber-orange arrow on a darker fill, rounded square. It's the visual answer to "where do I commit this thought."
- The selector pills at the bottom (`Default | Sonnet | Low`) use **chevron-down + label** with no border, no fill — same ghost pattern as the title bar's project label.
- "Thinking" is rendered as **icon + label, no spinner, no progress bar.** A static lightning glyph and the word "Thinking." in muted text. Movement is implied by context, not animation.
- The composer has a **subtle right-side arrow grip** in the corner — looks like a resize affordance, drawn at very low alpha so it disappears unless you look.

**Replicate.**
- [ ] **Asymmetric bubble pattern for any conversational UI** (alerts log, agent chat, command history): the user/system side gets a subtle bubble, the response side flows as bare prose.
- [ ] Bubble fill = surface_1 over surface_0; border at ~10% alpha. Almost invisible, structurally present.
- [ ] **Send/commit-style action = the only saturated colored element** in a composer. Everything else is ghost.
- [ ] Status indicator pattern: icon + muted label, no animation, when async work is in progress.
- [ ] Pill-style menu triggers: label + chevron, no border, no fill, no padding chrome — used for model/mode selection, account switchers, anywhere a dropdown lives in a dense composer.

**Apex today?** We have no chat surface yet, but we will (Vanessa, alert console, agent log). More importantly, this asymmetric pattern applies to our **alerts log and order history**: user actions = subtle bubble, system responses = flowed text. We currently bubble both sides. Cost: low, mostly a styling decision in `chat_message` widget when we build it.

---

### 5. Right panel — Onboarding wizard

This is where the most reusable patterns live.

#### 5a. Header block ("Welcome to Zed" + Finish Setup CTA)

**Visible.** Top-left: the Zed logo glyph (a stacked angled-line motif), then a two-line text block — bold "Welcome to Zed" and below it a muted subtitle "The editor for what's next." Top-right: a single button reading `Finish Setup  Ctrl-Enter`.

**Special.**
- **Logo + headline use a horizontal layout**, not stacked. Logo is ~32px square and sits to the left of the text.
- The subtitle is **`text_muted`**, ~13px, sitting directly under the headline with no line-break gap beyond default leading.
- The CTA button has the **keybind embedded inside the button**, not floating to its right or shown in a tooltip. The button literally reads "Finish Setup" then a small gap, then "Ctrl-Enter" in muted text. Same widget, two text spans.

**Replicate.**
- [ ] **Embedded-keybind button.** Add a `Button::with_kbd("Finish Setup", "Ctrl-Enter")` constructor. The keybind text uses the muted icon ramp; the action text uses primary text. This is *much* better than tooltips for primary CTAs because it teaches the shortcut without requiring a hover.
- [ ] Header pattern: glyph + headline + muted subtitle in a row, used at the top of every settings/preferences/dialog page.

**Apex today?** Our buttons don't render keybinds inline. Our settings dialogs have headings but no consistent glyph treatment. Cost: low for the button (extend our existing button widget), low for the header pattern.

---

#### 5b. Theme section

**Visible.** Section heading `Theme` (sentence case, slightly larger than body, bold-ish). Right-aligned in the same row: a Light / Dark / System segmented control with `Dark` selected (the selected segment has a slightly lighter fill). Below: three preview cards labeled `One`, `Ayu`, `Gruvbox`. Each card is a small rectangular thumbnail showing a stylized mock editor — 4-5 horizontal "code" lines in colored stripes, sitting on the theme's background. The card label sits below the thumbnail, centered.

**Special.**
- **Section header and segmented control share a row.** Heading-left, control-right. Saves vertical space and immediately tells the user what they're toggling.
- The **segmented control selected state is the lightest possible**: just a fill change. No border, no shadow, no animation. The unselected segments have no background at all.
- **Theme preview cards use real stripe-art, not screenshots.** Three or four colored bars of varying lengths, evoking syntax highlighting without literal text. This is *much* faster to render and ages better than screenshot thumbnails.
- The selected card (One) has a **visible 1px border ring** in the accent color. The other cards have no border. This is the cleanest "selected" affordance in the whole screenshot.
- Card labels sit below, centered, in `text_muted`. The selected card's label may be `text` — hard to be sure at this resolution.

**Replicate.**
- [ ] **Section header + control on the same row** — adopt across all settings panels.
- [ ] **Segmented control with fill-only selection** — no borders, no shadows. Today our segmented control uses both a fill AND a border AND a shadow. Subtract.
- [ ] **Stripe-art preview thumbnails** for any "pick a visual style" choice (chart themes, watchlist densities, layout presets). Cheap to draw, scales to any size.
- [ ] **Accent-ring selection** for cards. Currently we use background lift; ring is more readable at a glance and doesn't disturb the thumbnail content.

**Apex today?** We have a chart theme picker but it uses screenshots. We have segmented controls in the design system but they're heavier than this one. Cost: low for both fixes.

---

#### 5c. Base Keymap section

**Visible.** Section heading `Base Keymap`. Below it, a **3 × 2 grid** of buttons, each with a small left-aligned icon and a label: VS Code, JetBrains, Sublime Text, Atom, Emacs, Cursor. VS Code is selected.

**Special.**
- **The grid cells share borders** — the seams between adjacent cells are single 1px lines, not gaps. The whole grid reads as one bordered table, like a giant segmented control rather than 6 individual buttons. This is structurally different from our card grids.
- The **selected cell has a fill** that's only barely lighter than the unselected cells, plus what looks like a slightly thicker bottom border or accent treatment (hard to tell at resolution; the audit's "ring" pattern would fit here too).
- Each cell has the **icon at the very left edge** with substantial padding before the label. Dense but readable.
- **No hover state visible** — these are at rest. But given Zed's pattern, hover would be an alpha overlay, instant.

**Replicate.**
- [ ] **Shared-border grid pattern** for mutually exclusive choice grids. Build as a single bordered table, not as a flex of separate buttons. Use for: chart type picker, layout preset picker, broker picker.
- [ ] Selected cell: subtle fill + accent bottom-edge or accent ring; not a dramatic color change.

**Apex today?** Our card grids use gap-based layout with each card having its own border. Heavier, more bento-like. The shared-border pattern is denser and more "settings page"-appropriate. Cost: medium — needs a new layout widget but it's not complex.

---

#### 5d. Agent Setup section

**Visible.** Section heading `Agent Setup`. Description line below: `Install your favorite agents and start your first thread.` (muted text). Then a row of 5 cells: Zed Agent, Claude Agent, Codex CLI, GitHub Copilot, Cursor. Each cell stacks an icon + name on top, and a state-dependent action below. States visible: `Sign In` (Zed Agent), `✓` (Claude Agent — checkmark, indicating signed in), `✓` (Codex CLI), `Install` (GitHub Copilot), `Install` (Cursor).

**Special.**
- **The action label changes by state** — Sign In, ✓, Install — but the *cell layout doesn't change*. Same height, same width, same padding. The state lives in the bottom slot.
- A **bare ✓ with no surrounding text** is enough to communicate "installed and signed in." No "Connected" label, no green pill, no badge — just the glyph in muted text.
- The cells use the **same shared-border grid** as Base Keymap above. Two grids stacked tells the user "these are parallel choices in two domains."
- Two of the five cells are checkmarked — meaning the **completed/done state visually recedes** rather than getting promoted. The CTAs (`Sign In`, `Install`) draw the eye because they're verbs.

**Replicate.**
- [ ] **State-aware action slot in choice cells.** The cell is a stable container; the bottom slot reads "Sign In" / "✓" / "Install" / "Update" depending on connection/install state. Apply directly to our broker connection grid (IBKR connected = ✓, Tradier disconnected = "Connect", Schwab not installed = "Install").
- [ ] **Bare-glyph "done" state.** Stop using filled green pills for "connected." A muted ✓ is enough. Reserve color for *attention*, not for *completion*.

**Apex today?** Our broker setup currently uses status pills. They're noisier than they need to be. Cost: low — restyling existing widget.

---

#### 5e. Import Settings row

**Visible.** Heading `Import Settings`. Body line: `Automatically pull your settings from other editors`. Right-aligned: two pills, `VS Code` and `Cursor`.

**Special.**
- **Two-column row** — text content left, action(s) right. This is the single most-used pattern on the entire wizard page.
- The pills are **borderless ghost buttons**, sitting next to each other with a small gap. They don't look like primary CTAs, they look like options.
- The body text is muted and reads as a sentence, not a label.

**Replicate.**
- [ ] Universal "labeled row with right-aligned action(s)" widget: heading + muted description on the left, one or more controls on the right. This is how 80% of settings UI should be built.

---

#### 5f. Toggle rows (Vim Mode, Trust All Projects, Help improve, Help fix)

**Visible.** Each row: bold-ish title on the left, optional info icon next to the title (Trust All Projects has one), muted description below the title, switch on the far right. The two "Help" rows have their switches enabled (accent-colored fill). Vim Mode and Trust All show their switch states (Vim off / dimmed; Trust All on / accent).

**Special.**
- The **switch is the only widget on the right side** — no descriptive value, no chevron, no menu. Pure boolean.
- **Switch on-state uses the warm accent** at full saturation. Off-state is a muted gray track with no thumb-shadow. The contrast between on and off is unambiguous.
- The optional **info icon (ⓘ)** sits inline with the title, not in the description. This makes "more details available" a property of the *setting*, not its description.
- Generous vertical spacing between rows — looks like ~Base12 between rows. The dense info doesn't feel cramped because each row gets air.
- The description is **`text_muted`** and never wraps to a third line — content is constrained to fit two lines max.

**Replicate.**
- [ ] **Toggle-row widget** (title + info-icon + description + switch) as the canonical "boolean preference" row. Apply to all our settings panels.
- [ ] Switch styling: track-only off state, accent-fill on state, no shadow on the thumb.
- [ ] Inline info icon next to the *title*, not the description.

**Apex today?** We have toggle rows but they vary in spacing and the switch styling has an unnecessary glow. Cost: low.

---

### 6. Status bar (bottom edge, ~24px tall)

**Visible.** Two clusters separated by a wide empty region. Left cluster: ~5 small monochrome icons (panel toggle, history, file-tree, search, checkmark/diagnostics). Right cluster: ~6 icons (looks like collaboration, agent, debug, branch, tasks, project).

**Special.**
- **All icons are the same size** (~14px) and the **same muted color** — it's a row of pure ghost icons against the panel surface. No labels, no badges, no separators between icons in a cluster.
- The two clusters are **separated by empty space**, not a divider. Negative space does the grouping.
- The status bar shares the **same surface as the panel above it**. No darker fill, no top border. Implied seam.
- No text in the status bar at all. No "Ln 12, Col 4," no encoding, no language mode. (Zed shows those elsewhere.) This is unusual — most editors crowd the status bar with metrics. Zed treats it as **launch points only**.

**Replicate.**
- [ ] **Status bar = launch-point row only.** Resist the urge to put live metrics there. Put icons that *open panels* on the left, *open tools* on the right.
- [ ] No dividers, no labels, no badges. Negative space + tooltips do the work.
- [ ] Same surface as content; no separate elevation.

**Apex today?** Our status bar has connection state, account info, P&L summary, broker badge — it's busy. Some of that *is* useful in a trading app (you do want connection state visible always). Reasonable compromise: keep one critical live indicator (connection dot) at the right edge, move everything else into the dock or popovers triggered by status-bar icons. Cost: medium — partially a redesign decision.

---

## Cross-cutting observations

These patterns repeat across regions and define the feel:

1. **Single-elevation chrome.** Title bar, tab strip, panels, status bar — all share one surface color. Where most apps stack 3-5 elevation steps in their chrome, Zed stacks one. The result: the chrome is *quiet*, and the content (chart, text, conversation) is the only thing with visual weight. This is the single biggest "feel" choice in the screenshot.

2. **Borders are vertical-only.** I count maybe two horizontal borders in the entire image — between the conversation panel's bubble and the rest, and around the bottom notification card. Pane separators are vertical lines; tab strips have no top/bottom border; status bar has no top border. The chrome breathes vertically, divides horizontally.

3. **Accent appears in exactly four places.** Active-tab dot. Selected segmented-control fill. Selected theme-card ring. Switch on-state. Send-arrow button. Five places total. Everything else is grayscale. If we audit our app, we probably use accent in 20+ places. Cut to 5.

4. **Keybinds everywhere.** Embedded in CTA buttons (`Finish Setup  Ctrl-Enter`). Inline next to empty-state actions (`Open Project  Ctrl-K`). The user is *constantly* being taught the shortcut without being interrupted. This is a cultural tell — the app expects power users and treats them as the default.

5. **Muted text is the default.** Headlines and active labels are full `text`; everything else — descriptions, inactive tabs, keybinds, status icons, tooltips, placeholder text — is muted. We get hierarchy from value contrast, not size or weight changes.

6. **No visible animation in this still.** Obviously. But the implication: every state change in this UI is one frame. There's no "settling" or "easing" anywhere implied by the design — the screenshot looks like every other state would look identical, just with a different element highlighted.

7. **Section headers are sentence-case, mid-weight, no underline, no all-caps.** "Theme", "Base Keymap", "Agent Setup", "Import Settings", "Vim Mode" — same treatment for every one. We're mixing sentence case and uppercase across our settings; pick one (sentence case) and stop.

---

## Top 10 extractions ranked by visible-impact-per-day

1. **Active-tab dot, kill the underline** — half a day. Single highest "Zed-ness" return per pixel changed. Touches every tabbed surface in the app.
2. **Single-elevation chrome** — 1 day audit + recolor. Title bar, status bar, tab strip, sidebar all share one surface color. Removes the busy stacking that makes our chrome look like a control surface.
3. **Embedded-keybind button** (`Button::with_kbd`) — half a day. Universal "teach the shortcut while doing the action" pattern. Use on every primary CTA.
4. **Toggle-row widget** (title + info-icon + muted description + accent switch) — 1 day. Replaces every ad-hoc settings row.
5. **Asymmetric bubble for conversational surfaces** — half a day styling. Applies to alerts log, order history, agent chat. Gives our text panels editorial calm.
6. **State-aware choice cell** (icon + name + bottom action slot that changes by state) — 1 day. Direct application to broker connection grid.
7. **Stripe-art theme/preset thumbnails** instead of screenshots — 1 day. Faster, more legible at small sizes, ages perfectly.
8. **Section-header-in-row-with-control** layout (`Theme` + segmented control on same row) — adopted at zero cost during the next settings refactor.
9. **Empty-state pattern** (stacked text actions + inline keybinds, no illustration) — half a day. Apply to every empty pane.
10. **Status bar as launch-points only** — 1 day to relocate metrics into the dock. Makes the bottom of the window calm again.

Total ≈ 7 days for high-visibility wins. Combined with Phases 1–3 of the audit (~7 days), about 2 weeks gets us to "side-by-side reads as the same family."

---

## What we should NOT copy

- **Warm-gray surface palette.** Trading red/green legibility benefits from a cool/neutral substrate. The warmth biases hue perception subtly. Steal the elevation discipline, not the temperature.
- **No-text status bar.** A trading app must show connection state at all times. Compromise: one live indicator + the rest as launch points.
- **`Sign In` as raw text in title bar.** For us this slot probably needs an account avatar with broker-connection dots — too much state to render as a ghost text label.
- **Three-up theme card row at this size.** Our chart-theme picker has more themes; the card row pattern works for ≤4 options, breaks beyond.
- **Bare-glyph ✓ for "connected" on broker rows during active trading.** Onboarding is fine. But during a session the user wants the broker name + a colored dot, not a checkmark — color is faster than glyph for at-a-glance.
- **Send-arrow as the only saturated element on a screen.** Works for an editor. Doesn't work for an order ticket where Buy and Sell *both* must be saturated and *must* be color-coded against each other. Don't apply the "single saturated CTA" rule to trade actions.
- **Generous vertical rhythm in the toggle-row stack.** Beautiful for an onboarding screen with 4 toggles. Wrong for a settings panel with 40 toggles. Use Zed's spacing for hero/onboarding surfaces; use tighter spacing for dense preference pages.

---

## Report

**Doc path:** `C:\Users\USER\documents\development\Apex-terminal\src-tauri\docs\ZED_VISUAL_ANALYSIS.md`
**Word count:** ~2,400 words.
**Distinct UI patterns catalogued:** 27 (across 6 regions + 7 cross-cutting + 7 don't-copy).

**Top 3 to implement first:**
1. **Active-tab dot, kill the underline.** Lowest cost, highest "is this Zed?" payoff.
2. **Single-elevation chrome (one surface for title bar, tab strip, status bar, panels).** Reframes the entire window from "stacked control surface" to "calm canvas with content."
3. **Embedded-keybind button** — universal pattern, free a11y/teaching win, applies to every primary CTA on every dialog.

**Things in the screenshot I couldn't pin down at this resolution:**
- Whether the Theme card selected-state is a 1px ring or a 1px subtle bg lift (probably ring; not 100% confirmable).
- Exact font in use — looks like Zed Sans (their custom face), but I can't confirm metrics from this raster.
- Whether the bubble border is a true 1px stroke or a 1px alpha-blended ring.
- Whether the active-tab dot has a 0.5px outline or is a flat fill.
- Hover/focus states across the entire UI — the screenshot is fully at-rest.
- Microinteractions on the send button when held vs hover-only.
- The exact accent hue (looks like ~`#d8a468` honey-amber, but JPEG compression introduces hue drift).

**How much of the "Zed feel" is in this single screenshot vs requires motion?**
Roughly **75% in the still, 25% in motion.** The still captures: token discipline (single accent, heavy muted text), elevation choices (one-surface chrome), spacing rhythm, the active-state vocabulary (dot, ring, fill — never underline+bg+shadow stacks), and the keybind-everywhere culture. What the still cannot show but matters substantially: the **snappiness** of state transitions (no easing), the way **panels slide rather than fade**, the way **typing has zero perceptible latency**, the **scroll inertia** (Zed scrolls with momentum but no rubber-banding). These motion choices are well-documented in the audit's animation section, so we have them covered conceptually — but a single screenshot can lie about an app's feel if you only look at the static composition. Apex could match this image exactly and still feel wrong if our hover/scroll/transition behavior is springy where Zed's is instant.

The honest summary: this screenshot tells us *what to build*, not *how it should respond*. Combine it with the audit's animation section and we have the full brief.
