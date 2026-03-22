import { CandleRenderer, GridRenderer, LineRenderer, VolumeRenderer } from '../renderer'
import type { GPUContext } from './types'
import { CoordSystem } from './types'
import type { ColumnStore } from '../data/columns'
import type { IndicatorSnapshot, IndicatorOutput } from '../indicators'

export class PaneContext {
  dirty = true
  needsReconfigure = false
  data: ColumnStore | null = null
  indicators: IndicatorSnapshot | null = null
  viewport: { viewStart: number; viewCount: number; cs: CoordSystem } | null = null

  private device: GPUDevice
  private format: GPUTextureFormat
  gpuContext: GPUCanvasContext
  private candle: CandleRenderer
  private grid: GridRenderer
  private volume: VolumeRenderer
  private lineRenderers: LineRenderer[] = []
  private indicatorOutputs: IndicatorOutput[] = []
  private showVolume = true
  private resizeTimer: number | null = null
  private markDirtyFn: () => void
  private destroyed = false
  /** Tracks what changed for potential partial upload optimization */
  lastAction: 'updated' | 'created' | null = null

  constructor(
    readonly id: string,
    readonly canvas: HTMLCanvasElement,
    ctx: GPUContext,
    markDirty: () => void,
  ) {
    this.device = ctx.device
    this.format = ctx.format
    this.markDirtyFn = markDirty

    this.gpuContext = canvas.getContext('webgpu') as GPUCanvasContext
    this.gpuContext.configure({ device: ctx.device, format: ctx.format, alphaMode: 'premultiplied' })

    this.candle = new CandleRenderer(ctx)
    this.grid = new GridRenderer(ctx)
    this.volume = new VolumeRenderer(ctx)
  }

  setViewport(v: { viewStart: number; viewCount: number; cs: CoordSystem }): void {
    if (this.destroyed) return
    this.viewport = v
    this.dirty = true
    this.markDirtyFn()
  }

  setData(d: ColumnStore, indicators: IndicatorSnapshot, action?: 'updated' | 'created' | null): void {
    if (this.destroyed) return
    this.data = d
    this.indicators = indicators
    this.lastAction = action ?? null
    this.dirty = true
    this.markDirtyFn()
  }

  setVisibility(showVol: boolean, outputs: IndicatorOutput[]): void {
    if (this.destroyed) return
    this.showVolume = showVol
    this.indicatorOutputs = outputs
    this.dirty = true
    this.markDirtyFn()
  }

  resize(width: number, height: number): void {
    if (this.destroyed) return
    if (this.resizeTimer) clearTimeout(this.resizeTimer)
    this.resizeTimer = window.setTimeout(() => {
      if (this.destroyed) return
      const dpr = window.devicePixelRatio || 1
      const pw = Math.round(width * dpr)
      const ph = Math.round(height * dpr)
      if (this.canvas.width === pw && this.canvas.height === ph) return
      this.canvas.width = pw
      this.canvas.height = ph
      this.canvas.style.width = width + 'px'
      this.canvas.style.height = height + 'px'
      this.gpuContext.configure({ device: this.device, format: this.format, alphaMode: 'premultiplied' })
      this.dirty = true
      this.markDirtyFn()
    }, 16)
  }

  render(): GPUCommandBuffer {
    const encoder = this.device.createCommandEncoder()
    const view = this.gpuContext.getCurrentTexture().createView()
    const pass = encoder.beginRenderPass({
      colorAttachments: [{
        view, loadOp: 'clear',
        clearValue: { r: 0.05, g: 0.05, b: 0.05, a: 1 },
        storeOp: 'store',
      }],
    })

    if (this.viewport?.cs && this.data) {
      const { cs, viewStart, viewCount } = this.viewport

      // Volume bars first (behind everything)
      if (this.showVolume) {
        this.volume.upload(this.data, cs, viewStart, viewCount)
        this.volume.render(pass)
      }

      // Grid
      this.grid.upload(cs)
      this.grid.render(pass)

      // Candles
      this.candle.upload(this.data, cs, viewStart, viewCount)
      this.candle.render(pass)

      // Dynamic indicator lines
      this.ensureLineRenderers(this.indicatorOutputs.length)
      for (let i = 0; i < this.indicatorOutputs.length; i++) {
        const out = this.indicatorOutputs[i]
        this.lineRenderers[i].upload(out.values, cs, viewStart, viewCount, out.color, out.width)
        this.lineRenderers[i].render(pass)
      }
    }

    pass.end()
    this.lastAction = null
    return encoder.finish()
  }

  reconfigure(ctx: GPUContext): void {
    if (this.destroyed) return
    if (!this.canvas.isConnected) return
    this.device = ctx.device
    this.format = ctx.format
    this.gpuContext.configure({ device: ctx.device, format: ctx.format, alphaMode: 'premultiplied' })
    this.destroyRenderers()
    this.candle = new CandleRenderer(ctx)
    this.grid = new GridRenderer(ctx)
    this.volume = new VolumeRenderer(ctx)
    this.lineRenderers = []
    this.needsReconfigure = false
    this.dirty = true
  }

  destroy(): void {
    if (this.destroyed) return
    this.destroyed = true
    if (this.resizeTimer) clearTimeout(this.resizeTimer)
    this.destroyRenderers()
  }

  /** Ensure we have enough LineRenderers for the current indicator outputs */
  private ensureLineRenderers(needed: number): void {
    const ctx = { device: this.device, format: this.format }
    while (this.lineRenderers.length < needed) {
      this.lineRenderers.push(new LineRenderer(ctx))
    }
  }

  private destroyRenderers(): void {
    try { this.candle.destroy() } catch (e) { /* */ }
    try { this.grid.destroy() } catch (e) { /* */ }
    try { this.volume.destroy() } catch (e) { /* */ }
    for (const l of this.lineRenderers) {
      try { l.destroy() } catch (e) { /* */ }
    }
    this.lineRenderers = []
  }
}
