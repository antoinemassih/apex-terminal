import type { GPUContext } from '../engine/types'
import { CoordSystem } from '../engine/types'
import { ColumnStore } from '../data/columns'
import shaderSrc from './shaders/candles.wgsl?raw'

const FLOATS_PER_INSTANCE = 10
const VERTS_PER_CANDLE = 18
const BULL_COLOR = [0.18, 0.78, 0.45, 1.0]
const BEAR_COLOR = [0.93, 0.27, 0.27, 1.0]

export class CandleRenderer {
  private pipeline: GPURenderPipeline
  private uniformBuffer: GPUBuffer
  private uniformBindGroup: GPUBindGroup
  private instanceBuffer: GPUBuffer | null = null
  private instanceCount = 0
  private readonly device: GPUDevice

  constructor(ctx: GPUContext) {
    this.device = ctx.device
    this.uniformBuffer = ctx.device.createBuffer({
      size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    })

    const module = ctx.device.createShaderModule({ code: shaderSrc })
    const bgl = ctx.device.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: {} }],
    })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: ctx.device.createPipelineLayout({ bindGroupLayouts: [bgl] }),
      vertex: {
        module, entryPoint: 'vs_main',
        buffers: [{
          arrayStride: FLOATS_PER_INSTANCE * 4,
          stepMode: 'instance',
          attributes: [
            { shaderLocation: 0, offset: 0,  format: 'float32' },
            { shaderLocation: 1, offset: 4,  format: 'float32' },
            { shaderLocation: 2, offset: 8,  format: 'float32' },
            { shaderLocation: 3, offset: 12, format: 'float32' },
            { shaderLocation: 4, offset: 16, format: 'float32' },
            { shaderLocation: 5, offset: 20, format: 'float32' },
            { shaderLocation: 6, offset: 24, format: 'float32x4' },
          ],
        }],
      },
      fragment: { module, entryPoint: 'fs_main', targets: [{ format: ctx.format }] },
      primitive: { topology: 'triangle-list' },
    })

    this.uniformBindGroup = ctx.device.createBindGroup({
      layout: bgl,
      entries: [{ binding: 0, resource: { buffer: this.uniformBuffer } }],
    })
  }

  upload(data: ColumnStore, cs: CoordSystem, viewStart: number, viewCount: number) {
    const end = Math.min(viewStart + viewCount, data.length)
    const count = end - viewStart
    if (count <= 0) return

    const arr = new Float32Array(count * FLOATS_PER_INSTANCE)
    const bodyW = cs.clipBarWidth() * 0.5
    const wickW = Math.max(bodyW * 0.15, 0.001)

    for (let i = 0; i < count; i++) {
      const di = viewStart + i
      const base = i * FLOATS_PER_INSTANCE
      const isBull = data.closes[di] >= data.opens[di]
      const color = isBull ? BULL_COLOR : BEAR_COLOR

      arr[base + 0] = cs.barToClipX(i)
      arr[base + 1] = cs.priceToClipY(data.opens[di])
      arr[base + 2] = cs.priceToClipY(data.closes[di])
      arr[base + 3] = cs.priceToClipY(data.lows[di])
      arr[base + 4] = cs.priceToClipY(data.highs[di])
      arr[base + 5] = bodyW
      arr[base + 6] = color[0]
      arr[base + 7] = color[1]
      arr[base + 8] = color[2]
      arr[base + 9] = color[3]
    }

    if (this.instanceBuffer) this.instanceBuffer.destroy()
    this.instanceBuffer = this.device.createBuffer({
      size: arr.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    })
    this.device.queue.writeBuffer(this.instanceBuffer, 0, arr)
    this.instanceCount = count
    this.device.queue.writeBuffer(this.uniformBuffer, 0, new Float32Array([wickW, 0, 0, 0]))
  }

  render(pass: GPURenderPassEncoder) {
    if (!this.instanceBuffer || this.instanceCount === 0) return
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.uniformBindGroup)
    pass.setVertexBuffer(0, this.instanceBuffer)
    pass.draw(VERTS_PER_CANDLE, this.instanceCount)
  }

  destroy() {
    this.instanceBuffer?.destroy()
    this.uniformBuffer.destroy()
  }
}
