/**
 * Chart theme system for apex-terminal.
 *
 * Each theme provides a complete color palette for every visual element:
 * candles, volumes, wicks, grid, axes, crosshair, OHLC labels, and chrome.
 *
 * GPU renderers consume the `bull` / `bear` / `bullVolume` / `bearVolume` /
 * `wick` colors as RGBA float arrays ([r, g, b, a] in 0-1 range).
 * All hex strings are provided for Canvas 2D / CSS usage.
 */

// ---------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------

export interface ChartTheme {
  /** Human-readable display name */
  name: string

  /** Canvas / CSS background color (hex) */
  background: string

  /** Bullish candle body color (hex) */
  bull: string
  /** Bearish candle body color (hex) */
  bear: string

  /** Bullish volume bar color -- should be semi-transparent (hex with alpha or rgba) */
  bullVolume: string
  /** Bearish volume bar color -- should be semi-transparent (hex with alpha or rgba) */
  bearVolume: string

  /** Wick / shadow color (hex) */
  wick: string

  /** Grid line color (hex) */
  grid: string

  /** Axis text / tick color (hex) */
  axisText: string

  /** Crosshair line + label background color (hex) */
  crosshair: string

  /** OHLC label text color (hex) */
  ohlcLabel: string

  /** Active pane border color (hex) */
  borderActive: string
  /** Inactive pane border color (hex) */
  borderInactive: string

  /** Toolbar background (hex) */
  toolbarBackground: string
  /** Toolbar border (hex) */
  toolbarBorder: string
  /** Watchlist / gutter background — defaults to toolbarBackground if omitted */
  watchlistBackground?: string

  // ------- Pre-computed GPU-friendly RGBA float arrays -------

  /** Bullish candle body [r, g, b, a] in 0-1 */
  bullRGBA: readonly [number, number, number, number]
  /** Bearish candle body [r, g, b, a] in 0-1 */
  bearRGBA: readonly [number, number, number, number]
  /** Bullish volume bar [r, g, b, a] in 0-1 (alpha < 1) */
  bullVolumeRGBA: readonly [number, number, number, number]
  /** Bearish volume bar [r, g, b, a] in 0-1 (alpha < 1) */
  bearVolumeRGBA: readonly [number, number, number, number]
  /** Wick [r, g, b, a] in 0-1 */
  wickRGBA: readonly [number, number, number, number]
  /** Grid [r, g, b, a] in 0-1 */
  gridRGBA: readonly [number, number, number, number]
  /** Axis color [r, g, b, a] in 0-1 */
  axisRGBA: readonly [number, number, number, number]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Convert a hex color (#RGB, #RRGGBB, or #RRGGBBAA) to an RGBA float tuple. */
function hexToRGBA(hex: string, alphaOverride?: number): [number, number, number, number] {
  let r = 0, g = 0, b = 0, a = 1
  const h = hex.replace('#', '')
  if (h.length === 3) {
    r = parseInt(h[0] + h[0], 16) / 255
    g = parseInt(h[1] + h[1], 16) / 255
    b = parseInt(h[2] + h[2], 16) / 255
  } else if (h.length === 6) {
    r = parseInt(h.substring(0, 2), 16) / 255
    g = parseInt(h.substring(2, 4), 16) / 255
    b = parseInt(h.substring(4, 6), 16) / 255
  } else if (h.length === 8) {
    r = parseInt(h.substring(0, 2), 16) / 255
    g = parseInt(h.substring(2, 4), 16) / 255
    b = parseInt(h.substring(4, 6), 16) / 255
    a = parseInt(h.substring(6, 8), 16) / 255
  }
  if (alphaOverride !== undefined) a = alphaOverride
  return [r, g, b, a]
}

/** Build a full ChartTheme from the hex-only fields. */
function buildTheme(partial: Omit<
  ChartTheme,
  'bullRGBA' | 'bearRGBA' | 'bullVolumeRGBA' | 'bearVolumeRGBA' |
  'wickRGBA' | 'gridRGBA' | 'axisRGBA'
>): ChartTheme {
  return {
    ...partial,
    bullRGBA: hexToRGBA(partial.bull),
    bearRGBA: hexToRGBA(partial.bear),
    bullVolumeRGBA: hexToRGBA(partial.bull, 0.25),
    bearVolumeRGBA: hexToRGBA(partial.bear, 0.25),
    wickRGBA: hexToRGBA(partial.wick),
    gridRGBA: hexToRGBA(partial.grid),
    axisRGBA: hexToRGBA(partial.axisText),
  }
}

// ---------------------------------------------------------------------------
// Theme definitions
// ---------------------------------------------------------------------------

const midnight = buildTheme({
  name: 'Midnight',
  background: '#0d0d0d',
  bull: '#2ecc71',
  bear: '#e74c3c',
  bullVolume: '#2ecc7140',
  bearVolume: '#e74c3c40',
  wick: '#555555',
  grid: '#262626',
  axisText: '#666666',
  crosshair: '#1a1a2e',
  ohlcLabel: '#cccccc',
  borderActive: '#2a6496',
  borderInactive: '#1a1a1a',
  toolbarBackground: '#111111',
  toolbarBorder: '#222222',
})

const nord = buildTheme({
  name: 'Nord',
  background: '#2e3440',
  bull: '#a3be8c',
  bear: '#bf616a',
  bullVolume: '#a3be8c40',
  bearVolume: '#bf616a40',
  wick: '#4c566a',
  grid: '#3b4252',
  axisText: '#81a1c1',
  crosshair: '#434c5e',
  ohlcLabel: '#d8dee9',
  borderActive: '#88c0d0',
  borderInactive: '#3b4252',
  toolbarBackground: '#2e3440',
  toolbarBorder: '#3b4252',
  watchlistBackground: '#242932',
})

const monokai = buildTheme({
  name: 'Monokai',
  background: '#272822',
  bull: '#a6e22e',
  bear: '#f92672',
  bullVolume: '#a6e22e40',
  bearVolume: '#f9267240',
  wick: '#75715e',
  grid: '#3e3d32',
  axisText: '#a59f85',
  crosshair: '#49483e',
  ohlcLabel: '#f8f8f2',
  borderActive: '#e6db74',
  borderInactive: '#3e3d32',
  toolbarBackground: '#1e1f1c',
  toolbarBorder: '#3e3d32',
})

const solarizedDark = buildTheme({
  name: 'Solarized Dark',
  background: '#002b36',
  bull: '#859900',
  bear: '#dc322f',
  bullVolume: '#85990040',
  bearVolume: '#dc322f40',
  wick: '#586e75',
  grid: '#073642',
  axisText: '#839496',
  crosshair: '#073642',
  ohlcLabel: '#93a1a1',
  borderActive: '#2aa198',
  borderInactive: '#073642',
  toolbarBackground: '#002b36',
  toolbarBorder: '#073642',
  watchlistBackground: '#00202b',
})

const dracula = buildTheme({
  name: 'Dracula',
  background: '#282a36',
  bull: '#50fa7b',
  bear: '#ff5555',
  bullVolume: '#50fa7b40',
  bearVolume: '#ff555540',
  wick: '#6272a4',
  grid: '#343746',
  axisText: '#bd93f9',
  crosshair: '#44475a',
  ohlcLabel: '#f8f8f2',
  borderActive: '#ff79c6',
  borderInactive: '#343746',
  toolbarBackground: '#21222c',
  toolbarBorder: '#343746',
})

const gruvbox = buildTheme({
  name: 'Gruvbox',
  background: '#282828',
  bull: '#b8bb26',
  bear: '#fb4934',
  bullVolume: '#b8bb2640',
  bearVolume: '#fb493440',
  wick: '#665c54',
  grid: '#3c3836',
  axisText: '#d5c4a1',
  crosshair: '#3c3836',
  ohlcLabel: '#ebdbb2',
  borderActive: '#fe8019',
  borderInactive: '#3c3836',
  toolbarBackground: '#1d2021',
  toolbarBorder: '#3c3836',
})

const catppuccin = buildTheme({
  name: 'Catppuccin',
  background: '#1e1e2e',
  bull: '#a6e3a1',
  bear: '#f38ba8',
  bullVolume: '#a6e3a140',
  bearVolume: '#f38ba840',
  wick: '#585b70',
  grid: '#313244',
  axisText: '#b4befe',
  crosshair: '#313244',
  ohlcLabel: '#cdd6f4',
  borderActive: '#cba6f7',
  borderInactive: '#313244',
  toolbarBackground: '#181825',
  toolbarBorder: '#313244',
})

const tokyoNight = buildTheme({
  name: 'Tokyo Night',
  background: '#1a1b26',
  bull: '#9ece6a',
  bear: '#f7768e',
  bullVolume: '#9ece6a40',
  bearVolume: '#f7768e40',
  wick: '#565f89',
  grid: '#24283b',
  axisText: '#7aa2f7',
  crosshair: '#292e42',
  ohlcLabel: '#c0caf5',
  borderActive: '#7dcfff',
  borderInactive: '#24283b',
  toolbarBackground: '#16161e',
  toolbarBorder: '#24283b',
})

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

export const THEMES: Record<string, ChartTheme> = {
  midnight,
  nord,
  monokai,
  'solarized-dark': solarizedDark,
  dracula,
  gruvbox,
  catppuccin,
  'tokyo-night': tokyoNight,
}

/** All available theme keys. */
export const THEME_NAMES = Object.keys(THEMES) as ReadonlyArray<string>

/** Retrieve a theme by name, falling back to midnight if not found. */
export function getTheme(name: string): ChartTheme {
  return THEMES[name] ?? THEMES['midnight']
}
