import type { GPUContext } from '../engine/types'
import { CoordSystem } from '../engine/types'
import shaderSrc from './shaders/line.wgsl?raw'

export class LineRenderer {
  private pipeline: GPURenderPipeline
  private uniformBuffer: GPUBuffer
  private bindGroup: GPUBindGroup
  private pointBuffer: GPUBuffer | null = null
  private pointBufferSize = 0 // track allocated size for reuse
  private segmentCount = 0
  private readonly device: GPUDevice

  constructor(ctx: GPUContext) {
    this.device = ctx.device
    this.uniformBuffer = ctx.device.createBuffer({
      size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    })
    const module = ctx.device.createShaderModule({ code: shaderSrc })
    const bgl = ctx.device.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: {} }],
    })
    this.pipeline = ctx.device.createRenderPipeline({
      layout: ctx.device.createPipelineLayout({ bindGroupLayouts: [bgl] }),
      vertex: {
        module, entryPoint: 'vs_main',
        buffers: [
          { arrayStride: 8, stepMode: 'instance',
            attributes: [{ shaderLocation: 0, offset: 0, format: 'float32x2' }] },
          { arrayStride: 8, stepMode: 'instance',
            attributes: [{ shaderLocation: 1, offset: 0, format: 'float32x2' }] },
        ],
      },
      fragment: {
        module, entryPoint: 'fs_main',
        targets: [{
          format: ctx.format,
          blend: {
            color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha', operation: 'add' },
            alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
          },
        }],
      },
      primitive: { topology: 'triangle-list' },
    })
    this.bindGroup = ctx.device.createBindGroup({
      layout: bgl,
      entries: [{ binding: 0, resource: { buffer: this.uniformBuffer } }],
    })
  }

  upload(values: Float64Array, cs: CoordSystem, viewStart: number, viewCount: number,
         color: [number, number, number, number], lineWidthPx: number) {
    const points: number[] = []
    for (let i = 0; i < viewCount; i++) {
      const di = viewStart + i
      if (di >= values.length || isNaN(values[di])) continue
      points.push(cs.barToClipX(i), cs.priceToClipY(values[di]))
    }
    if (points.length < 4) { this.segmentCount = 0; return }

    const data = new Float32Array(points)
    const neededBytes = data.byteLength

    // Reuse existing buffer if it's large enough, avoiding GPU allocation per frame
    if (this.pointBuffer && this.pointBufferSize >= neededBytes) {
      this.device.queue.writeBuffer(this.pointBuffer, 0, data)
    } else {
      // Need a new buffer — destroy old one first
      if (this.pointBuffer) this.pointBuffer.destroy()
      // Allocate with 50% headroom to reduce future reallocations
      const allocSize = Math.ceil(neededBytes * 1.5)
      this.pointBuffer = this.device.createBuffer({
        size: allocSize, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      })
      this.pointBufferSize = allocSize
      this.device.queue.writeBuffer(this.pointBuffer, 0, data)
    }

    this.segmentCount = (points.length / 2) - 1

    const clipWidth = (lineWidthPx / cs.width) * 2
    this.device.queue.writeBuffer(this.uniformBuffer, 0,
      new Float32Array([clipWidth, 0, 0, 0, ...color]))
  }

  render(pass: GPURenderPassEncoder) {
    if (!this.pointBuffer || this.segmentCount <= 0) return
    // Only use the portion of the buffer that has valid data
    const usedBytes = (this.segmentCount + 1) * 8 // each point is 2 floats = 8 bytes
    const segmentBytes = usedBytes - 8
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.bindGroup)
    pass.setVertexBuffer(0, this.pointBuffer, 0, segmentBytes)
    pass.setVertexBuffer(1, this.pointBuffer, 8, segmentBytes)
    pass.draw(6, this.segmentCount)
  }

  destroy() {
    this.pointBuffer?.destroy()
    this.pointBuffer = null
    this.pointBufferSize = 0
    this.uniformBuffer.destroy()
  }
}
