/**
 * Drawing repository backed by PostgreSQL via Tauri IPC commands.
 * Replaces LocalDrawingRepository (IndexedDB) for server-side persistence.
 */

import { invoke } from '@tauri-apps/api/core'
import type { Drawing, Point } from '../types'
import type { DrawingRepository } from './DrawingRepository'

export class TauriDrawingRepository implements DrawingRepository {
  async loadAll(): Promise<Drawing[]> {
    try {
      const rows = await invoke<Drawing[]>('drawings_load_all')
      return rows.map(fixDrawing)
    } catch (e) {
      console.error('Failed to load drawings from DB:', e)
      return []
    }
  }

  async loadForSymbol(symbol: string): Promise<Drawing[]> {
    try {
      const rows = await invoke<Drawing[]>('drawings_load_symbol', { symbol })
      return rows.map(fixDrawing)
    } catch (e) {
      console.error(`Failed to load drawings for ${symbol}:`, e)
      return []
    }
  }

  async save(drawing: Drawing): Promise<void> {
    try {
      await invoke('drawings_save', {
        drawing: {
          id: drawing.id,
          symbol: drawing.symbol,
          timeframe: drawing.timeframe,
          type: drawing.type,
          points: drawing.points,
          color: drawing.color,
          opacity: drawing.opacity,
          lineStyle: drawing.lineStyle,
          thickness: drawing.thickness,
        }
      })
    } catch (e) {
      console.error('Failed to save drawing:', e)
    }
  }

  async updatePoints(id: string, points: Point[]): Promise<void> {
    try {
      await invoke('drawings_update_points', { id, points })
    } catch (e) {
      console.error('Failed to update drawing points:', e)
    }
  }

  async updateStyle(id: string, style: Partial<Pick<Drawing, 'color' | 'opacity' | 'lineStyle' | 'thickness'>>): Promise<void> {
    try {
      await invoke('drawings_update_style', {
        id,
        color: style.color ?? null,
        opacity: style.opacity ?? null,
        lineStyle: style.lineStyle ?? null,
        thickness: style.thickness ?? null,
      })
    } catch (e) {
      console.error('Failed to update drawing style:', e)
    }
  }

  async remove(id: string): Promise<void> {
    try {
      await invoke('drawings_remove', { id })
    } catch (e) {
      console.error('Failed to remove drawing:', e)
    }
  }

  async clear(): Promise<void> {
    try {
      await invoke('drawings_clear')
    } catch (e) {
      console.error('Failed to clear drawings:', e)
    }
  }
}

/** Normalize field names from Rust (snake_case) to TypeScript (camelCase) */
function fixDrawing(d: any): Drawing {
  return {
    id: d.id,
    symbol: d.symbol,
    timeframe: d.timeframe,
    type: d.drawing_type ?? d.type,
    points: d.points ?? [],
    color: d.color ?? '#4a9eff',
    opacity: d.opacity ?? 1,
    lineStyle: d.line_style ?? d.lineStyle ?? 'solid',
    thickness: d.thickness ?? 1.5,
  }
}
