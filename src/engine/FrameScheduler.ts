import type { PaneContext } from './PaneContext'

export class FrameScheduler {
  private panes = new Map<string, PaneContext>()
  private rafId: number | null = null
  private running = false
  private paused = false
  private device: GPUDevice

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

  private tick(): void {
    this.rafId = null

    if (this.paused) {
      // Keep loop alive during recovery so resume() doesn't need to restart
      if (this.running) this.rafId = requestAnimationFrame(() => this.tick())
      return
    }

    const commandBuffers: GPUCommandBuffer[] = []

    for (const [id, pane] of this.panes) {
      if (!pane.dirty) continue
      this.renderPane(id, commandBuffers)
    }

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
