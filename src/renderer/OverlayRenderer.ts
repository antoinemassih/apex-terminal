import type { GPUContext } from '../engine/types'
import shaderSrc from './shaders/overlay.wgsl?raw'

/**
 * GPU overlay renderer — renders arbitrary anti-aliased line segments.
 *
 * Used for crosshair, trendlines, hlines, hzone borders.
 * Each line is 48 bytes (12 f32s) in a storage buffer.
 * Per-frame CPU cost: writeBuffer of only the lines that changed.
 *
 * Replaces Canvas2D for line rendering — text labels stay on Canvas2D.
 */

const FLOATS_PER_LINE = 12
const BYTES_PER_LINE = FLOATS_PER_LINE * 4
const MAX_LINES = 512
const VERTS_PER_LINE = 6

export interface OverlayLine {
  x0: number; y0: number  // clip-space start
  x1: number; y1: number  // clip-space end
  r: number; g: number; b: number; a: number  // color
  dashLen: number  // 0 = solid
  gapLen: number
  width: number    // clip-space width
}

export class OverlayRenderer {
  private readonly pipeline: GPURenderPipeline
  private readonly bgl: GPUBindGroupLayout
  private readonly storageBuf: GPUBuffer
  private readonly bindGroup: GPUBindGroup
  private readonly cpuData: Float32Array
  private lineCount = 0
  private readonly device: GPUDevice

  constructor(ctx: GPUContext) {
    this.device = ctx.device

    this.cpuData = new Float32Array(MAX_LINES * FLOATS_PER_LINE)

    this.storageBuf = ctx.device.createBuffer({
      size: MAX_LINES * BYTES_PER_LINE,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    })

    const module = ctx.device.createShaderModule({ code: shaderSrc })

    this.bgl = ctx.device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: 'read-only-storage' } },
      ],
    })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: ctx.device.createPipelineLayout({ bindGroupLayouts: [this.bgl] }),
      vertex: { module, entryPoint: 'vs_main' },
      fragment: {
        module,
        entryPoint: 'fs_main',
        targets: [{
          format: ctx.format,
          blend: {
            color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha' },
            alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
          },
        }],
      },
      primitive: { topology: 'triangle-list' },
    })

    this.bindGroup = ctx.device.createBindGroup({
      layout: this.bgl,
      entries: [
        { binding: 0, resource: { buffer: this.storageBuf } },
      ],
    })
  }

  /**
   * Upload overlay lines for this frame.
   * Call once per frame with the full set of lines to render.
   */
  upload(lines: OverlayLine[]): void {
    const n = Math.min(lines.length, MAX_LINES)
    this.lineCount = n
    if (n === 0) return

    for (let i = 0; i < n; i++) {
      const l = lines[i]
      const o = i * FLOATS_PER_LINE
      this.cpuData[o]     = l.x0
      this.cpuData[o + 1] = l.y0
      this.cpuData[o + 2] = l.x1
      this.cpuData[o + 3] = l.y1
      this.cpuData[o + 4] = l.r
      this.cpuData[o + 5] = l.g
      this.cpuData[o + 6] = l.b
      this.cpuData[o + 7] = l.a
      this.cpuData[o + 8] = l.dashLen
      this.cpuData[o + 9] = l.gapLen
      this.cpuData[o + 10] = l.width
      this.cpuData[o + 11] = 0
    }

    this.device.queue.writeBuffer(this.storageBuf, 0, this.cpuData, 0, n * FLOATS_PER_LINE)
  }

  /** Render within an existing render pass. */
  render(pass: GPURenderPassEncoder): void {
    if (this.lineCount === 0) return
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.bindGroup)
    pass.draw(VERTS_PER_LINE, this.lineCount)
  }

  destroy(): void {
    this.storageBuf.destroy()
  }
}
