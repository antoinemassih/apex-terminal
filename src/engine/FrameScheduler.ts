import type { PaneContext } from './PaneContext'
import type { FrameStats } from './types'

const RING_SIZE = 120

export class FrameScheduler {
  private panes = new Map<string, PaneContext>()
  private rafId: number | null = null
  private running = false
  private paused = false
  private device: GPUDevice

  // Timing: render duration (not inter-frame gap)
  private renderTimes = new Float64Array(RING_SIZE)
  private renderIdx = 0
  private lastRenderPanes = 0
  // Update rate tracking
  private frameCount = 0
  private frameCountStart = 0
  private updatesPerSec = 0

  constructor(device: GPUDevice) {
    this.device = device
    this.frameCountStart = performance.now()
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
    const filled = Math.min(this.renderIdx, RING_SIZE)
    let avg = 0, peak = 0
    if (filled > 0) {
      let sum = 0
      for (let i = 0; i < filled; i++) {
        const t = this.renderTimes[i]
        sum += t
        if (t > peak) peak = t
      }
      avg = sum / filled
    }

    return {
      updatesPerSec: this.updatesPerSec,
      renderTimeMs: avg,
      renderTimePeak: peak,
      panesRendered: this.lastRenderPanes,
      panesTotal: this.panes.size,
    }
  }

  private tick(): void {
    this.rafId = null

    if (this.paused) {
      if (this.running) this.rafId = requestAnimationFrame(() => this.tick())
      return
    }

    // Measure render time (not gap between frames)
    const t0 = performance.now()

    const commandBuffers: GPUCommandBuffer[] = []
    let dirtyCount = 0

    for (const [id, pane] of this.panes) {
      if (!pane.dirty) continue
      dirtyCount++
      this.renderPane(id, commandBuffers)
    }
    this.lastRenderPanes = dirtyCount

    if (commandBuffers.length > 0) {
      try {
        this.device.queue.submit(commandBuffers)
      } catch (e) {
        console.error('GPU submit failed:', e)
      }
    }

    const t1 = performance.now()
    if (dirtyCount > 0) {
      this.renderTimes[this.renderIdx % RING_SIZE] = t1 - t0
      this.renderIdx++

      // Update rate: count frames in 1-second windows
      this.frameCount++
      const elapsed = t1 - this.frameCountStart
      if (elapsed >= 1000) {
        this.updatesPerSec = Math.round(this.frameCount * 1000 / elapsed)
        this.frameCount = 0
        this.frameCountStart = t1
      }
    }

    // Schedule next frame only if there's still dirty work
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
      pane.dirty = false
    }
  }
}
