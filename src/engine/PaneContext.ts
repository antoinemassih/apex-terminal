import { CandleRenderer, LineRenderer, VolumeRenderer } from '../renderer'
import { GpuBarBuffer } from '../renderer/GpuBarBuffer'
import { GpuLineBuffer } from '../renderer/GpuLineBuffer'
import { PriceRangeCompute } from '../renderer/PriceRangeCompute'
import type { GPUContext } from './types'
import { CoordSystem } from './types'
import type { ColumnStore } from '../data/columns'
import type { IndicatorSnapshot, IndicatorOutput } from '../indicators'
import type { ChartTheme } from '../themes'
import { getTheme } from '../themes'

function hexToGPUColor(hex: string): { r: number; g: number; b: number; a: number } {
  const h = hex.replace('#', '')
  return {
    r: parseInt(h.substring(0, 2), 16) / 255,
    g: parseInt(h.substring(2, 4), 16) / 255,
    b: parseInt(h.substring(4, 6), 16) / 255,
    a: 1,
  }
}

export class PaneContext {
  dirty = true
  needsReconfigure = false
  data: ColumnStore | null = null
  indicators: IndicatorSnapshot | null = null
  viewport: { viewStart: number; viewCount: number; cs: CoordSystem } | null = null

  private device: GPUDevice
  private format: GPUTextureFormat
  gpuContext: GPUCanvasContext

  /** GPU-resident OHLCV storage — permanent, never re-uploaded on pan/zoom */
  private gpuBars: GpuBarBuffer
  private candle: CandleRenderer
  private volume: VolumeRenderer

  /**
   * Parallel arrays — one entry per visible indicator line.
   * gpuLineBuffers[i] is owned exclusively by lineRenderers[i].
   * Both arrays grow together and are destroyed together.
   */
  private gpuLineBuffers: GpuLineBuffer[] = []
  private lineRenderers: LineRenderer[] = []
  private indicatorOutputs: IndicatorOutput[] = []

  private priceRangeCompute: PriceRangeCompute
  private showVolume = true
  private theme: ChartTheme = getTheme('midnight')
  private resizeTimer: number | null = null
  private markDirtyFn: () => void
  private destroyed = false
  lastAction: 'updated' | 'created' | null = null

  constructor(
    readonly id: string,
    readonly canvas: HTMLCanvasElement,
    ctx: GPUContext,
    markDirty: () => void,
  ) {
    this.device      = ctx.device
    this.format      = ctx.format
    this.markDirtyFn = markDirty

    this.gpuContext = canvas.getContext('webgpu') as GPUCanvasContext
    this.gpuContext.configure({ device: ctx.device, format: ctx.format, alphaMode: 'premultiplied' })

    this.gpuBars           = new GpuBarBuffer(ctx.device)
    this.candle            = new CandleRenderer(ctx, this.gpuBars)
    this.volume            = new VolumeRenderer(ctx, this.gpuBars)
    this.priceRangeCompute = new PriceRangeCompute(ctx)
  }

  /** GPU-computed price range from last frame (1-frame delay). Null until first result. */
  get gpuPriceRange(): { min: number; max: number } | null {
    return this.priceRangeCompute.priceRange
  }

  setViewport(v: { viewStart: number; viewCount: number; cs: CoordSystem }): void {
    if (this.destroyed) return
    this.viewport = v
    this.dirty = true
    this.markDirtyFn()
  }

  setData(d: ColumnStore, indicators: IndicatorSnapshot, action?: 'updated' | 'created' | null): void {
    if (this.destroyed) return
    this.data       = d
    this.indicators = indicators
    this.lastAction = action ?? null

    // ── OHLCV: minimal GPU write ────────────────────────────────────────
    const evicted = d.lastEvictCount > 0
    if (action === 'updated') {
      this.gpuBars.updateLastBar(d)             // 24 bytes
    } else if (action === 'created' && !evicted) {
      this.gpuBars.appendBar(d, d.length - 1)  // 24 bytes (or full if buffer grew)
    } else {
      this.gpuBars.load(d)                      // full reload on symbol/TF change or eviction
    }

    // ── Indicator lines: incremental GPU write ──────────────────────────
    // setVisibility() is NOT called on ticks — update buffers here instead.
    // indicatorOutputs[i].values points to the live Float64Array in IndicatorEngine
    // (updated in-place), so we always read the latest value.
    const n = Math.min(this.indicatorOutputs.length, this.gpuLineBuffers.length)
    if (action === 'updated') {
      for (let i = 0; i < n; i++)
        this.gpuLineBuffers[i].updateLast(this.indicatorOutputs[i].values)  // 4 bytes each
    } else if (action === 'created' && !evicted) {
      for (let i = 0; i < n; i++)
        this.gpuLineBuffers[i].appendValue(this.indicatorOutputs[i].values) // 4 bytes each
    } else if (action === 'created') {
      // Eviction shifted all bars — full reload to keep GPU in sync
      for (let i = 0; i < n; i++)
        this.gpuLineBuffers[i].load(this.indicatorOutputs[i].values)
    }
    // null action → setVisibility() will be called separately (symbol/TF change)

    this.dirty = true
    this.markDirtyFn()
  }

  /**
   * Called when indicator visibility or configuration changes (not on every tick).
   * Always does a full GPU load of current indicator values.
   */
  setVisibility(showVol: boolean, outputs: IndicatorOutput[]): void {
    if (this.destroyed) return
    this.showVolume       = showVol
    this.indicatorOutputs = outputs

    // Ensure we have a GpuLineBuffer + LineRenderer for each output
    this.ensureLineBuffersAndRenderers(outputs.length)

    // Full load — values may have changed since last setVisibility (symbol/TF change, etc.)
    for (let i = 0; i < outputs.length; i++) {
      this.gpuLineBuffers[i].load(outputs[i].values)
    }

    this.dirty = true
    this.markDirtyFn()
  }

  setTheme(themeName: string): void {
    if (this.destroyed) return
    this.theme = getTheme(themeName)
    this.dirty = true
    this.markDirtyFn()
  }

  resize(width: number, height: number): void {
    if (this.destroyed) return
    if (this.resizeTimer) clearTimeout(this.resizeTimer)
    this.resizeTimer = window.setTimeout(() => {
      if (this.destroyed) return
      const dpr = window.devicePixelRatio || 1
      const pw  = Math.round(width  * dpr)
      const ph  = Math.round(height * dpr)
      if (this.canvas.width === pw && this.canvas.height === ph) return
      this.canvas.width  = pw
      this.canvas.height = ph
      this.canvas.style.width  = width  + 'px'
      this.canvas.style.height = height + 'px'
      this.gpuContext.configure({ device: this.device, format: this.format, alphaMode: 'premultiplied' })
      this.dirty = true
      this.markDirtyFn()
    }, 16)
  }

  /**
   * Encode the price-range compute pass into its own command buffer.
   * Kept separate from render() so a compute validation error can never
   * contaminate the render command buffer.
   * Returns null when there is nothing to dispatch.
   */
  buildComputeCommands(): GPUCommandBuffer | null {
    if (!this.viewport?.cs || !this.data) return null
    const encoder = this.device.createCommandEncoder()
    this.priceRangeCompute.dispatch(encoder, this.gpuBars, this.viewport.viewStart, this.viewport.viewCount)
    return encoder.finish()
  }

  render(): GPUCommandBuffer {
    const encoder = this.device.createCommandEncoder()
    const view    = this.gpuContext.getCurrentTexture().createView()
    const bg      = hexToGPUColor(this.theme.background)
    const pass    = encoder.beginRenderPass({
      colorAttachments: [{
        view, loadOp: 'clear', clearValue: bg, storeOp: 'store',
      }],
    })

    if (this.viewport?.cs && this.data) {
      const { cs, viewStart, viewCount } = this.viewport
      const t = this.theme

      // Volume bars (behind candles)
      if (this.showVolume) {
        this.volume.upload(this.data, cs, viewStart, viewCount, t.bullVolumeRGBA, t.bearVolumeRGBA)
        this.volume.render(pass)
      }

      // Candles — 80-byte uniform write; vertex shader computes all clip-space
      this.candle.upload(cs, viewStart, viewCount, t.bullRGBA, t.bearRGBA)
      this.candle.render(pass)

      // Indicator lines — 64-byte uniform write per line; values stay on GPU
      for (let i = 0; i < this.indicatorOutputs.length; i++) {
        if (i >= this.lineRenderers.length) break
        const out = this.indicatorOutputs[i]
        this.lineRenderers[i].upload(cs, viewStart, viewCount, out.color, out.width, this.data.length)
        this.lineRenderers[i].render(pass)
      }
    }

    pass.end()
    this.lastAction = null
    return encoder.finish()
  }

  /** Call after device.queue.submit() to trigger async GPU→CPU price range readback. */
  postSubmit(): void {
    this.priceRangeCompute.postSubmit()
  }

  reconfigure(ctx: GPUContext): void {
    if (this.destroyed) return
    if (!this.canvas.isConnected) return
    this.device = ctx.device
    this.format = ctx.format
    this.gpuContext.configure({ device: ctx.device, format: ctx.format, alphaMode: 'premultiplied' })

    // Destroy all GPU resources tied to the old device
    this.destroyRenderers()   // clears lineRenderers[] and gpuLineBuffers[]
    this.gpuBars.destroy()
    this.priceRangeCompute.destroy()

    // Recreate everything with new device
    this.gpuBars           = new GpuBarBuffer(ctx.device)
    this.priceRangeCompute = new PriceRangeCompute(ctx)
    if (this.data) this.gpuBars.load(this.data)
    this.candle  = new CandleRenderer(ctx, this.gpuBars)
    this.volume  = new VolumeRenderer(ctx, this.gpuBars)

    // Restore indicator line buffers from cached indicatorOutputs
    for (const out of this.indicatorOutputs) {
      const buf = new GpuLineBuffer(ctx.device)
      this.gpuLineBuffers.push(buf)
      this.lineRenderers.push(new LineRenderer(ctx, buf))
      buf.load(out.values)
    }

    this.needsReconfigure = false
    this.dirty = true
  }

  destroy(): void {
    if (this.destroyed) return
    this.destroyed = true
    if (this.resizeTimer) clearTimeout(this.resizeTimer)
    this.destroyRenderers()
    this.gpuBars.destroy()
    this.priceRangeCompute.destroy()
  }

  /**
   * Grows gpuLineBuffers[] and lineRenderers[] together.
   * Arrays only grow — excess entries render 0 segments and are harmless.
   */
  private ensureLineBuffersAndRenderers(needed: number): void {
    const ctx = { device: this.device, format: this.format }
    while (this.gpuLineBuffers.length < needed) {
      const buf = new GpuLineBuffer(this.device)
      this.gpuLineBuffers.push(buf)
      this.lineRenderers.push(new LineRenderer(ctx, buf))
    }
  }

  private destroyRenderers(): void {
    try { this.candle.destroy() } catch { /* */ }
    try { this.volume.destroy() } catch { /* */ }
    for (const l of this.lineRenderers)    try { l.destroy()   } catch { /* */ }
    for (const b of this.gpuLineBuffers)   try { b.destroy()   } catch { /* */ }
    this.lineRenderers  = []
    this.gpuLineBuffers = []
  }
}
