import type { PaneContext } from './PaneContext'

export class FrameScheduler {
  private panes = new Map<string, PaneContext>()
  private rafId: number | null = null
  private running = false
  private paused = false
  private device: GPUDevice
  activePaneId: string | null = null

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
      this.rafId = requestAnimationFrame(() => this.tick())
      return
    }

    const commandBuffers: GPUCommandBuffer[] = []

    // Active pane first for perceived responsiveness
    if (this.activePaneId) {
      const active = this.panes.get(this.activePaneId)
      if (active?.dirty) this.renderPane(this.activePaneId, commandBuffers)
    }

    for (const [id, pane] of this.panes) {
      if (id === this.activePaneId) continue
      if (!pane.dirty) continue
      this.renderPane(id, commandBuffers)
    }

    if (commandBuffers.length > 0) {
      try {
        this.device.queue.submit(commandBuffers)
      } catch (e) {
        console.error('GPU submit failed:', e)
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
