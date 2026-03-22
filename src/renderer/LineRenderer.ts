import type { GPUContext } from '../engine/types'
import { CoordSystem } from '../engine/types'
import shaderSrc from './shaders/line.wgsl?raw'

export class LineRenderer {
  private pipeline: GPURenderPipeline
  private uniformBuffer: GPUBuffer
  private bindGroup: GPUBindGroup
  private pointBuffer: GPUBuffer | null = null
  private pointBufferSize = 0
  private segmentCount = 0
  private readonly device: GPUDevice
  // Reusable CPU-side buffers — avoid allocation per frame
  private cpuPoints: Float32Array | null = null
  private readonly uniformData = new Float32Array(8) // [clipWidth, 0, 0, 0, r, g, b, a]

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
         color: [number, number, number, number], lineWidthPx: number, dataLength?: number) {
    // Ensure CPU buffer is large enough (2 floats per point, max viewCount points)
    const maxFloats = viewCount * 2
    if (!this.cpuPoints || this.cpuPoints.length < maxFloats) {
      this.cpuPoints = new Float32Array(Math.ceil(maxFloats * 1.5))
    }

    // Clamp to actual data length — prevents lines drawing into empty right margin
    const maxIdx = dataLength ?? values.length

    let count = 0
    for (let i = 0; i < viewCount; i++) {
      const di = viewStart + i
      if (di >= maxIdx || di >= values.length || isNaN(values[di])) continue
      this.cpuPoints[count++] = cs.barToClipX(i)
      this.cpuPoints[count++] = cs.priceToClipY(values[di])
    }
    if (count < 4) { this.segmentCount = 0; return }

    const neededBytes = count * 4

    // Reuse GPU buffer if large enough
    if (this.pointBuffer && this.pointBufferSize >= neededBytes) {
      this.device.queue.writeBuffer(this.pointBuffer, 0, this.cpuPoints, 0, count)
    } else {
      if (this.pointBuffer) this.pointBuffer.destroy()
      const allocSize = Math.ceil(neededBytes * 1.5)
      this.pointBuffer = this.device.createBuffer({
        size: allocSize, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      })
      this.pointBufferSize = allocSize
      this.device.queue.writeBuffer(this.pointBuffer, 0, this.cpuPoints, 0, count)
    }

    this.segmentCount = (count / 2) - 1

    // Write uniform without allocation
    const u = this.uniformData
    u[0] = (lineWidthPx / cs.width) * 2
    u[4] = color[0]; u[5] = color[1]; u[6] = color[2]; u[7] = color[3]
    this.device.queue.writeBuffer(this.uniformBuffer, 0, u)
  }

  render(pass: GPURenderPassEncoder) {
    if (!this.pointBuffer || this.segmentCount <= 0) return
    const usedBytes = (this.segmentCount + 1) * 8
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
    this.cpuPoints = null
  }
}
