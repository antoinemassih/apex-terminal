import type { GPUContext } from '../engine/types'
import type { CoordSystem } from '../chart/CoordSystem'
import type { GpuBarBuffer } from './GpuBarBuffer'
import shaderSrc from './shaders/candles_gpu.wgsl?raw'

const VERTS_PER_CANDLE = 18
/** 80 bytes / 4 = 20 f32 slots */
const VP_FLOATS = 20

// Muted teal green / warm red — closer to professional chart palettes
const BULL_COLOR = [0.033, 0.600, 0.506, 1.0]  // #089981
const BEAR_COLOR = [0.949, 0.212, 0.271, 1.0]  // #f23645

/**
 * Candle renderer — GPU-resident architecture.
 *
 * Raw OHLCV data lives permanently in GpuBarBuffer (STORAGE buffer).
 * Each render only writes an 80-byte Viewport uniform; all clip-space
 * math is done in the vertex shader with one FMA per price coord.
 *
 * CPU cost per frame: 80-byte writeBuffer + bind-group check.
 */
export class CandleRenderer {
  private readonly pipeline: GPURenderPipeline
  private readonly bgl: GPUBindGroupLayout
  private readonly uniformBuf: GPUBuffer
  private bindGroup: GPUBindGroup | null = null
  private lastStorageBuf: GPUBuffer | null = null
  private viewCount = 0
  /** Shared ArrayBuffer so Uint32 and Float32 views alias the same bytes */
  private readonly vpData = new Float32Array(VP_FLOATS)
  private readonly vpU32 = new Uint32Array(this.vpData.buffer)
  private readonly device: GPUDevice
  private readonly gpuBars: GpuBarBuffer

  constructor(ctx: GPUContext, gpuBars: GpuBarBuffer) {
    this.device  = ctx.device
    this.gpuBars = gpuBars

    this.uniformBuf = ctx.device.createBuffer({
      size: VP_FLOATS * 4,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    })

    const module = ctx.device.createShaderModule({ code: shaderSrc })

    this.bgl = ctx.device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: 'read-only-storage' } },
        { binding: 1, visibility: GPUShaderStage.VERTEX, buffer: { type: 'uniform' } },
      ],
    })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: ctx.device.createPipelineLayout({ bindGroupLayouts: [this.bgl] }),
      vertex:   { module, entryPoint: 'vs_main' },
      fragment: {
        module, entryPoint: 'fs_main',
        targets: [{
          format: ctx.format,
          // Alpha blend required for anti-aliased rounded-corner SDF edges
          blend: {
            color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha', operation: 'add' },
            alpha: { srcFactor: 'one',       dstFactor: 'one-minus-src-alpha', operation: 'add' },
          },
        }],
      },
      primitive: { topology: 'triangle-list' },
    })
  }

  /**
   * Write the 80-byte viewport uniform.  No bar data touched.
   * Called every frame when the pane is dirty (pan / zoom / tick / full load).
   */
  upload(cs: CoordSystem, viewStart: number, viewCount: number,
         bullColor?: readonly number[], bearColor?: readonly number[]): void {
    if (!this.gpuBars.valid) return

    // Rebuild bind group only when the storage buffer was reallocated
    if (this.gpuBars.buffer !== this.lastStorageBuf) {
      this.bindGroup = this.device.createBindGroup({
        layout: this.bgl,
        entries: [
          { binding: 0, resource: { buffer: this.gpuBars.buffer } },
          { binding: 1, resource: { buffer: this.uniformBuf } },
        ],
      })
      this.lastStorageBuf = this.gpuBars.buffer
    }

    const safeCount = Math.max(0,
      Math.min(viewCount, this.gpuBars.lenBars - viewStart))
    if (safeCount === 0) { this.viewCount = 0; return }

    // ── Viewport math (all on CPU, once per frame) ────────────────────
    const barStep        = cs.barStep
    const barStepClip    = barStep * 2 / cs.width
    const pixelOffsetFrac = cs.pixelOffset / barStep
    const bodyWidthClip  = cs.clipBarWidth() * 0.5
    const wickWidthClip  = Math.max(bodyWidthClip * 0.07, 1 / cs.width)
    const chartBotClip   = cs.priceToClipY(cs.minPrice)
    const chartTopClip   = cs.priceToClipY(cs.maxPrice)
    const priceRange     = cs.maxPrice - cs.minPrice
    // priceToClipY(p) = priceA + p * priceB  (one FMA in shader)
    const priceB         = priceRange > 0 ? (chartTopClip - chartBotClip) / priceRange : 0
    const priceA         = chartBotClip - cs.minPrice * priceB

    const u32 = this.vpU32
    const f32 = this.vpData
    const bc  = bullColor ?? BULL_COLOR
    const rc  = bearColor ?? BEAR_COLOR

    // offset  0: viewStart, viewCount, _pad, _pad
    u32[0] = viewStart;  u32[1] = safeCount;  u32[2] = 0;  u32[3] = 0
    // offset 16: barStepClip, pixelOffsetFrac, priceA, priceB
    f32[4] = barStepClip;  f32[5] = pixelOffsetFrac;  f32[6] = priceA;  f32[7] = priceB
    // offset 32: bodyWidthClip, wickWidthClip, canvasWidth, canvasHeight (physical px)
    const dpr = typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1
    f32[8] = bodyWidthClip;  f32[9] = wickWidthClip
    f32[10] = cs.width * dpr;  f32[11] = cs.height * dpr
    // offset 48: upColor
    f32[12] = bc[0];  f32[13] = bc[1];  f32[14] = bc[2];  f32[15] = bc[3]
    // offset 64: downColor
    f32[16] = rc[0];  f32[17] = rc[1];  f32[18] = rc[2];  f32[19] = rc[3]

    this.device.queue.writeBuffer(this.uniformBuf, 0, this.vpData)
    this.viewCount = safeCount
  }

  render(pass: GPURenderPassEncoder): void {
    if (!this.bindGroup || this.viewCount === 0) return
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.bindGroup)
    pass.draw(VERTS_PER_CANDLE, this.viewCount)
  }

  destroy(): void {
    this.uniformBuf.destroy()
    this.bindGroup      = null
    this.lastStorageBuf = null
    this.viewCount      = 0
  }
}
