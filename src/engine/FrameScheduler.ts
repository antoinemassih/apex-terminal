import type { PaneContext } from './PaneContext'
import type { FrameStats } from './types'

const RING_SIZE = 120

export class FrameScheduler {
  private panes = new Map<string, PaneContext>()
  private rafId: number | null = null
  private running = false
  private paused = false
  private device: GPUDevice

  private frameTimes = new Float64Array(RING_SIZE)
  private frameIdx = 0
  private lastFrameTime = 0
  private lastDirtyCount = 0

  constructor(device: GPUDevice) {
    this.device = device
  }

  addPane(pane: PaneContext): void { this.panes.set(pane.id, pane) }
  removePane(id: string): void { this.panes.delete(id) }

  start(): void {
    this.running = true
    if (!this.rafId) this.rafId = requestAnimationFrame(() => this.tick())
  }

  stop(): void {
    this.running = false
    if (this.rafId) { cancelAnimationFrame(this.rafId); this.rafId = null }
  }

  pause(): void { this.paused = true }

  resume(): void {
    this.paused = false
    if (this.running && !this.rafId) {
      this.rafId = requestAnimationFrame(() => this.tick())
    }
  }

  markDirty(paneId: string): void {
    const pane = this.panes.get(paneId)
    if (pane) pane.dirty = true
    if (!this.rafId && this.running && !this.paused) {
      this.rafId = requestAnimationFrame(() => this.tick())
    }
  }

  updateDevice(device: GPUDevice): void {
    this.device = device
  }

  getStats(): FrameStats {
    const filled = Math.min(this.frameIdx, RING_SIZE)
    if (filled === 0) return { fps: 0, frameTimeMs: 0, frameTimePeak: 0, dirtyPanes: this.lastDirtyCount }

    let sum = 0
    let peak = 0
    for (let i = 0; i < filled; i++) {
      const t = this.frameTimes[i]
      sum += t
      if (t > peak) peak = t
    }
    const avg = sum / filled
    const fps = avg > 0 ? 1000 / avg : 0

    return { fps, frameTimeMs: avg, frameTimePeak: peak, dirtyPanes: this.lastDirtyCount }
  }

  private tick(): void {
    this.rafId = null

    // Record frame timing
    const now = performance.now()
    if (this.lastFrameTime > 0) {
      this.frameTimes[this.frameIdx % RING_SIZE] = now - this.lastFrameTime
      this.frameIdx++
    }
    this.lastFrameTime = now

    if (this.paused) {
      // Keep loop alive during recovery so resume() doesn't need to restart
      if (this.running) this.rafId = requestAnimationFrame(() => this.tick())
      return
    }

    const commandBuffers: GPUCommandBuffer[] = []
    let dirtyCount = 0

    for (const [id, pane] of this.panes) {
      if (!pane.dirty) continue
      dirtyCount++
      this.renderPane(id, commandBuffers)
    }
    this.lastDirtyCount = dirtyCount

    if (commandBuffers.length > 0) {
      try {
        this.device.queue.submit(commandBuffers)
      } catch (e) {
        console.error('GPU submit failed:', e)
        // Don't crash — device.lost will fire and trigger recovery
      }
    }

    // Schedule next frame only if there's still dirty work or running continuously
    let hasDirty = false
    for (const pane of this.panes.values()) {
      if (pane.dirty) { hasDirty = true; break }
    }
    if (hasDirty && this.running) {
      this.rafId = requestAnimationFrame(() => this.tick())
    }
  }

  private renderPane(id: string, buffers: GPUCommandBuffer[]): void {
    const pane = this.panes.get(id)!
    try {
      buffers.push(pane.render())
      pane.dirty = false
    } catch (e) {
      console.warn(`Pane ${id} render failed:`, e)
      pane.dirty = false // don't retry broken frame endlessly
    }
  }
}
