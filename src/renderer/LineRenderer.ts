import type { GPUContext } from '../engine/types'
import type { CoordSystem } from '../chart/CoordSystem'
import type { GpuLineBuffer } from './GpuLineBuffer'
import shaderSrc from './shaders/line_gpu.wgsl?raw'

/** 64 bytes / 4 = 16 f32 slots */
const VP_FLOATS = 16

/**
 * Indicator line renderer — GPU-resident architecture.
 *
 * Indicator values live in GpuLineBuffer (STORAGE buffer) permanently.
 * Per-frame CPU work is a single 64-byte writeBuffer for the viewport uniform.
 * NaN gaps produce degenerate off-screen segments (same visual as old skip logic).
 *
 * Pan / zoom cost: 64 bytes written, zero indicator data touched.
 * Tick update cost: 4 bytes written to GpuLineBuffer, then 64-byte uniform.
 */
export class LineRenderer {
  private readonly pipeline: GPURenderPipeline
  private readonly bgl: GPUBindGroupLayout
  private readonly uniformBuf: GPUBuffer
  private bindGroup: GPUBindGroup | null = null
  private lastStorageBuf: GPUBuffer | null = null
  private segmentCount = 0
  private readonly vpData = new Float32Array(VP_FLOATS)
  private readonly vpU32 = new Uint32Array(this.vpData.buffer)
  private readonly device: GPUDevice
  private readonly gpuLine: GpuLineBuffer

  constructor(ctx: GPUContext, gpuLine: GpuLineBuffer) {
    this.device  = ctx.device
    this.gpuLine = gpuLine

    this.uniformBuf = ctx.device.createBuffer({
      size: VP_FLOATS * 4,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    })

    const module = ctx.device.createShaderModule({ code: shaderSrc })

    this.bgl = ctx.device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: 'read-only-storage' } },
        { binding: 1, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } },
      ],
    })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: ctx.device.createPipelineLayout({ bindGroupLayouts: [this.bgl] }),
      vertex: { module, entryPoint: 'vs_main' },
      fragment: {
        module, entryPoint: 'fs_main',
        targets: [{
          format: ctx.format,
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
   * Write the 64-byte viewport uniform.  No value data touched.
   * dataLength clamps drawing to the last actual bar (indicator arrays may be longer).
   */
  upload(cs: CoordSystem, viewStart: number, viewCount: number,
         color: [number, number, number, number], lineWidthPx: number,
         dataLength: number): void {
    if (!this.gpuLine.valid) return

    // Rebuild bind group only when the storage buffer was reallocated
    if (this.gpuLine.buffer !== this.lastStorageBuf) {
      this.bindGroup = this.device.createBindGroup({
        layout: this.bgl,
        entries: [
          { binding: 0, resource: { buffer: this.gpuLine.buffer } },
          { binding: 1, resource: { buffer: this.uniformBuf } },
        ],
      })
      this.lastStorageBuf = this.gpuLine.buffer
    }

    // Clamp end: don't draw past actual bar count or buffer length
    const safeEnd  = Math.min(viewStart + viewCount, this.gpuLine.lenFloats, dataLength)
    const segs     = Math.max(0, safeEnd - viewStart - 1)
    if (segs === 0) { this.segmentCount = 0; return }

    const barStep        = cs.barStep
    const barStepClip    = barStep * 2 / cs.width
    const pixelOffsetFrac = cs.pixelOffset / barStep
    const chartBotClip   = cs.priceToClipY(cs.minPrice)
    const chartTopClip   = cs.priceToClipY(cs.maxPrice)
    const priceRange     = cs.maxPrice - cs.minPrice
    const priceB         = priceRange > 0 ? (chartTopClip - chartBotClip) / priceRange : 0
    const priceA         = chartBotClip - cs.minPrice * priceB
    const lineWidthClip  = (lineWidthPx / cs.width) * 2

    const u32 = this.vpU32
    const f32 = this.vpData

    // offset  0: viewStart, segCount, _pad, _pad
    u32[0] = viewStart;  u32[1] = segs;  u32[2] = 0;  u32[3] = 0
    // offset 16: barStepClip, pixelOffsetFrac, priceA, priceB
    f32[4] = barStepClip;  f32[5] = pixelOffsetFrac;  f32[6] = priceA;  f32[7] = priceB
    // offset 32: lineWidthClip, _pad, _pad, _pad
    f32[8] = lineWidthClip;  f32[9] = 0;  f32[10] = 0;  f32[11] = 0
    // offset 48: color
    f32[12] = color[0];  f32[13] = color[1];  f32[14] = color[2];  f32[15] = color[3]

    this.device.queue.writeBuffer(this.uniformBuf, 0, this.vpData)
    this.segmentCount = segs
  }

  render(pass: GPURenderPassEncoder): void {
    if (!this.bindGroup || this.segmentCount <= 0) return
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.bindGroup)
    pass.draw(6, this.segmentCount)
  }

  destroy(): void {
    this.uniformBuf.destroy()
    this.bindGroup      = null
    this.lastStorageBuf = null
    this.segmentCount   = 0
  }
}
