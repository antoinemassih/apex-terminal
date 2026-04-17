/**
 * Font system for apex-terminal.
 *
 * 3 monospace options (for the data-dense traditional look) +
 * 3 beautiful sans-serif options (for a cleaner, modern UI feel).
 *
 * All fonts are loaded from Google Fonts via index.html.
 */

export interface AppFont {
  id: string
  /** Display name shown in the picker */
  name: string
  /** CSS font-family stack */
  stack: string
  category: 'monospace' | 'sans-serif'
}

export const FONTS: AppFont[] = [
  // ── Monospace ──────────────────────────────────────────────────────────────
  {
    id: 'jetbrains-mono',
    name: 'JetBrains Mono',
    stack: "'JetBrains Mono', 'Fira Code', monospace",
    category: 'monospace',
  },
  {
    id: 'fira-code',
    name: 'Fira Code',
    stack: "'Fira Code', 'JetBrains Mono', monospace",
    category: 'monospace',
  },
  {
    id: 'ibm-plex-mono',
    name: 'IBM Plex Mono',
    stack: "'IBM Plex Mono', 'JetBrains Mono', monospace",
    category: 'monospace',
  },

  // ── Sans-serif ─────────────────────────────────────────────────────────────
  {
    id: 'inter',
    name: 'Inter',
    stack: "'Inter', system-ui, -apple-system, sans-serif",
    category: 'sans-serif',
  },
  {
    id: 'plus-jakarta-sans',
    name: 'Plus Jakarta Sans',
    stack: "'Plus Jakarta Sans', 'Inter', sans-serif",
    category: 'sans-serif',
  },
  {
    id: 'ibm-plex-sans',
    name: 'IBM Plex Sans',
    stack: "'IBM Plex Sans', 'Inter', sans-serif",
    category: 'sans-serif',
  },
]

export const FONT_MAP: Record<string, AppFont> = Object.fromEntries(
  FONTS.map(f => [f.id, f])
)

export const DEFAULT_FONT_ID = 'jetbrains-mono'

export function getFont(id: string): AppFont {
  return FONT_MAP[id] ?? FONT_MAP[DEFAULT_FONT_ID]
}

/** Apply a font to the entire app by updating the CSS variable. */
export function applyFont(id: string) {
  const font = getFont(id)
  document.documentElement.style.setProperty('--font-app', font.stack)
}
