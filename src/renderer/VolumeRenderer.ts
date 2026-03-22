import type { GPUContext } from '../engine/types'
import { CoordSystem } from '../engine/types'
import type { ColumnStore } from '../data/columns'
import shaderSrc from './shaders/volume.wgsl?raw'

const FLOATS_PER_INSTANCE = 8 // x, height, bodyW, color(rgba), padding
const VERTS_PER_BAR = 6
const BULL_COLOR = [0.18, 0.78, 0.45, 0.25]
const BEAR_COLOR = [0.93, 0.27, 0.27, 0.25]

export class VolumeRenderer {
  private pipeline: GPURenderPipeline
  private instanceBuffer: GPUBuffer | null = null
  private instanceBufferSize = 0
  private instanceCount = 0
  private readonly device: GPUDevice
  private cpuBuffer: Float32Array | null = null

  constructor(ctx: GPUContext) {
    this.device = ctx.device
    const module = ctx.device.createShaderModule({ code: shaderSrc })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: 'auto',
      vertex: {
        module, entryPoint: 'vs_main',
        buffers: [{
          arrayStride: FLOATS_PER_INSTANCE * 4,
          stepMode: 'instance',
          attributes: [
            { shaderLocation: 0, offset: 0,  format: 'float32' },   // x_clip
            { shaderLocation: 1, offset: 4,  format: 'float32' },   // height
            { shaderLocation: 2, offset: 8,  format: 'float32' },   // body_w
            { shaderLocation: 3, offset: 12, format: 'float32x4' }, // color
          ],
        }],
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
  }

  upload(data: ColumnStore, cs: CoordSystem, viewStart: number, viewCount: number,
         bullColor?: readonly number[], bearColor?: readonly number[]) {
    const end = Math.min(viewStart + viewCount, data.length)
    const count = end - viewStart
    if (count <= 0) return

    // Find max volume in view for normalization
    let maxVol = 0
    for (let i = viewStart; i < end; i++) {
      if (data.volumes[i] > maxVol) maxVol = data.volumes[i]
    }
    if (maxVol === 0) maxVol = 1

    const floatsNeeded = count * FLOATS_PER_INSTANCE
    if (!this.cpuBuffer || this.cpuBuffer.length < floatsNeeded) {
      this.cpuBuffer = new Float32Array(Math.ceil(floatsNeeded * 1.5))
    }
    const arr = this.cpuBuffer
    const bodyW = cs.clipBarWidth() * 0.4

    for (let i = 0; i < count; i++) {
      const di = viewStart + i
      const base = i * FLOATS_PER_INSTANCE
      const isBull = data.closes[di] >= data.opens[di]
      const color = isBull ? (bullColor ?? BULL_COLOR) : (bearColor ?? BEAR_COLOR)

      arr[base + 0] = cs.barToClipX(i)
      arr[base + 1] = data.volumes[di] / maxVol // normalized 0-1
      arr[base + 2] = bodyW
      arr[base + 3] = color[0]
      arr[base + 4] = color[1]
      arr[base + 5] = color[2]
      arr[base + 6] = color[3]
      arr[base + 7] = 0 // padding to align stride
    }

    const byteLength = floatsNeeded * 4
    if (this.instanceBuffer && this.instanceBufferSize >= byteLength) {
      this.device.queue.writeBuffer(this.instanceBuffer, 0, arr, 0, floatsNeeded)
    } else {
      if (this.instanceBuffer) this.instanceBuffer.destroy()
      const allocSize = Math.ceil(byteLength * 1.5)
      this.instanceBuffer = this.device.createBuffer({
        size: allocSize, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      })
      this.instanceBufferSize = allocSize
      this.device.queue.writeBuffer(this.instanceBuffer, 0, arr, 0, floatsNeeded)
    }
    this.instanceCount = count
  }

  render(pass: GPURenderPassEncoder) {
    if (!this.instanceBuffer || this.instanceCount === 0) return
    pass.setPipeline(this.pipeline)
    pass.setVertexBuffer(0, this.instanceBuffer)
    pass.draw(VERTS_PER_BAR, this.instanceCount)
  }

  destroy() {
    this.instanceBuffer?.destroy()
    this.instanceBuffer = null
    this.instanceBufferSize = 0
    this.cpuBuffer = null
  }
}
