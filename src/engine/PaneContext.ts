import { CandleRenderer, GridRenderer, LineRenderer } from '../renderer'
import type { GPUContext } from './types'
import { CoordSystem } from './types'
import type { ColumnStore } from '../data/columns'
import type { IndicatorSnapshot } from '../indicators'

const LINE_CONFIGS = [
  { key: 'sma20' as const, color: [0.3, 0.6, 1.0, 0.8] as [number, number, number, number], width: 2.0 },
  { key: 'ema50' as const, color: [1.0, 0.6, 0.2, 0.8] as [number, number, number, number], width: 2.0 },
  { key: 'bbUpper' as const, color: [0.5, 0.5, 0.5, 0.4] as [number, number, number, number], width: 1.0 },
  { key: 'bbLower' as const, color: [0.5, 0.5, 0.5, 0.4] as [number, number, number, number], width: 1.0 },
]

export class PaneContext {
  dirty = true
  data: ColumnStore | null = null
  indicators: IndicatorSnapshot | null = null
  viewport: { viewStart: number; viewCount: number; cs: CoordSystem } | null = null

  private device: GPUDevice
  private format: GPUTextureFormat
  gpuContext: GPUCanvasContext
  private renderers: { candle: CandleRenderer; grid: GridRenderer; lines: LineRenderer[] }
  private resizeTimer: number | null = null
  private markDirtyFn: () => void

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

    this.renderers = {
      candle: new CandleRenderer(ctx),
      grid: new GridRenderer(ctx),
      lines: LINE_CONFIGS.map(() => new LineRenderer(ctx)),
    }
  }

  setViewport(v: { viewStart: number; viewCount: number; cs: CoordSystem }): void {
    this.viewport = v
    this.dirty = true
    this.markDirtyFn()
  }

  setData(d: ColumnStore, indicators: IndicatorSnapshot): void {
    this.data = d
    this.indicators = indicators
    this.dirty = true
    this.markDirtyFn()
  }

  resize(width: number, height: number): void {
    if (this.resizeTimer) clearTimeout(this.resizeTimer)
    this.resizeTimer = window.setTimeout(() => {
      const dpr = window.devicePixelRatio || 1
      this.canvas.width = Math.round(width * dpr)
      this.canvas.height = Math.round(height * dpr)
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

      this.renderers.grid.upload(cs)
      this.renderers.candle.upload(this.data, cs, viewStart, viewCount)

      if (this.indicators) {
        LINE_CONFIGS.forEach((cfg, i) => {
          const values = this.indicators![cfg.key]
          if (values) {
            this.renderers.lines[i].upload(values, cs, viewStart, viewCount, cfg.color, cfg.width)
          }
        })
      }

      this.renderers.grid.render(pass)
      this.renderers.candle.render(pass)
      for (const line of this.renderers.lines) line.render(pass)
    }

    pass.end()
    return encoder.finish()
  }

  reconfigure(ctx: GPUContext): void {
    if (!this.canvas.isConnected) return
    this.device = ctx.device
    this.format = ctx.format
    this.gpuContext.configure({ device: ctx.device, format: ctx.format, alphaMode: 'premultiplied' })
    this.renderers.candle.destroy()
    this.renderers.grid.destroy()
    for (const l of this.renderers.lines) l.destroy()
    this.renderers = {
      candle: new CandleRenderer(ctx),
      grid: new GridRenderer(ctx),
      lines: LINE_CONFIGS.map(() => new LineRenderer(ctx)),
    }
    this.dirty = true
  }

  destroy(): void {
    if (this.resizeTimer) clearTimeout(this.resizeTimer)
    this.renderers.candle.destroy()
    this.renderers.grid.destroy()
    for (const l of this.renderers.lines) l.destroy()
  }
}
