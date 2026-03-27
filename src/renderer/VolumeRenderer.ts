import type { GPUContext } from '../engine/types'
import type { CoordSystem } from '../chart/CoordSystem'
import type { ColumnStore } from '../data/columns'
import type { GpuBarBuffer } from './GpuBarBuffer'
import shaderSrc from './shaders/volume_gpu.wgsl?raw'

const VERTS_PER_BAR = 6
/** 80 bytes / 4 = 20 f32 slots */
const VP_FLOATS = 20

const BULL_COLOR = [0.18, 0.78, 0.45, 0.25]
const BEAR_COLOR = [0.93, 0.27, 0.27, 0.25]

/**
 * Volume renderer — GPU-resident architecture.
 *
 * Shares GpuBarBuffer with CandleRenderer.  Per-frame CPU work is limited
 * to an O(viewCount) max-volume scan (unavoidable for normalisation) plus
 * one 80-byte writeBuffer.
 */
export class VolumeRenderer {
  private readonly pipeline: GPURenderPipeline
  private readonly bgl: GPUBindGroupLayout
  private readonly uniformBuf: GPUBuffer
  private bindGroup: GPUBindGroup | null = null
  private lastStorageBuf: GPUBuffer | null = null
  private viewCount = 0
  private readonly vpData = new Float32Array(VP_FLOATS)
  private readonly vpU32 = new Uint32Array(this.vpData.buffer)
  private readonly device: GPUDevice
  // Max-volume cache — avoids O(viewCount) scan when only pixelOffset/price range changed
  private cachedMaxVol = 0
  private cachedMaxVolStart = -1
  private cachedMaxVolEnd = -1
  private cachedMaxVolDataLen = -1
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

  upload(data: ColumnStore, cs: CoordSystem, viewStart: number, viewCount: number,
         bullColor?: readonly number[], bearColor?: readonly number[]): void {
    if (!this.gpuBars.valid) return

    // Rebuild bind group if storage buffer was reallocated
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

    const end = Math.min(viewStart + viewCount, data.length)
    const safeCount = Math.max(0, end - viewStart)
    if (safeCount === 0) { this.viewCount = 0; return }

    // Max-volume scan — cached; only rescan when visible bar range or data changes
    let maxVol: number
    if (viewStart === this.cachedMaxVolStart && end === this.cachedMaxVolEnd && data.length === this.cachedMaxVolDataLen) {
      maxVol = this.cachedMaxVol
    } else {
      maxVol = 0
      for (let i = viewStart; i < end; i++) {
        if (data.volumes[i] > maxVol) maxVol = data.volumes[i]
      }
      if (maxVol === 0) maxVol = 1
      this.cachedMaxVol = maxVol
      this.cachedMaxVolStart = viewStart
      this.cachedMaxVolEnd = end
      this.cachedMaxVolDataLen = data.length
    }

    const barStep        = cs.barStep
    const barStepClip    = barStep * 2 / cs.width
    const pixelOffsetFrac = cs.pixelOffset / barStep
    const bodyWidthClip  = cs.clipBarWidth() * 0.4   // matches original sizing

    const u32 = this.vpU32
    const f32 = this.vpData
    const bc  = bullColor ?? BULL_COLOR
    const rc  = bearColor ?? BEAR_COLOR

    // offset  0: viewStart, viewCount, _pad, _pad
    u32[0] = viewStart;  u32[1] = safeCount;  u32[2] = 0;  u32[3] = 0
    // offset 16: barStepClip, pixelOffsetFrac, bodyWidthClip, maxVolume
    f32[4] = barStepClip;  f32[5] = pixelOffsetFrac;  f32[6] = bodyWidthClip;  f32[7] = maxVol
    // offset 32: volBottomClip, volHeightClip, _pad, _pad
    f32[8] = -1.0;  f32[9] = 0.3;  f32[10] = 0;  f32[11] = 0
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
    pass.draw(VERTS_PER_BAR, this.viewCount)
  }

  destroy(): void {
    this.uniformBuf.destroy()
    this.bindGroup      = null
    this.lastStorageBuf = null
    this.viewCount      = 0
  }
}
