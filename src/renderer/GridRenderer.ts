import type { GPUContext } from '../engine/types'
import { CoordSystem } from '../engine/types'
import shaderSrc from './shaders/grid.wgsl?raw'

const GRID_COLOR = [0.15, 0.15, 0.15, 1.0]
const AXIS_COLOR = [0.3, 0.3, 0.3, 1.0]
// Max grid lines: 8 price + ~20 time + 3 axes = ~31 lines = 62 vertices × 6 floats
const MAX_VERTS = 256
const FLOATS_PER_VERT = 6

export class GridRenderer {
  private pipeline: GPURenderPipeline
  private lineBuffer: GPUBuffer
  private vertexCount = 0
  private readonly device: GPUDevice
  // Reusable CPU buffer — never reallocated
  private readonly cpuBuffer = new Float32Array(MAX_VERTS * FLOATS_PER_VERT)

  constructor(ctx: GPUContext) {
    this.device = ctx.device
    const module = ctx.device.createShaderModule({ code: shaderSrc })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: 'auto',
      vertex: {
        module, entryPoint: 'vs_main',
        buffers: [{
          arrayStride: FLOATS_PER_VERT * 4,
          attributes: [
            { shaderLocation: 0, offset: 0, format: 'float32x2' },
            { shaderLocation: 1, offset: 8, format: 'float32x4' },
          ],
        }],
      },
      fragment: { module, entryPoint: 'fs_main', targets: [{ format: ctx.format }] },
      primitive: { topology: 'line-list' },
    })

    // Pre-allocate GPU buffer at max size — never recreated
    this.lineBuffer = ctx.device.createBuffer({
      size: MAX_VERTS * FLOATS_PER_VERT * 4,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    })
  }

  upload(cs: CoordSystem) {
    const buf = this.cpuBuffer
    let offset = 0

    const addLine = (x0: number, y0: number, x1: number, y1: number, color: number[]) => {
      if (offset + 12 > buf.length) return // safety
      const cx0 = (x0 / cs.width) * 2 - 1, cy0 = 1 - (y0 / cs.height) * 2
      const cx1 = (x1 / cs.width) * 2 - 1, cy1 = 1 - (y1 / cs.height) * 2
      buf[offset++] = cx0; buf[offset++] = cy0
      buf[offset++] = color[0]; buf[offset++] = color[1]; buf[offset++] = color[2]; buf[offset++] = color[3]
      buf[offset++] = cx1; buf[offset++] = cy1
      buf[offset++] = color[0]; buf[offset++] = color[1]; buf[offset++] = color[2]; buf[offset++] = color[3]
    }

    const priceStep = (cs.maxPrice - cs.minPrice) / 8
    for (let i = 0; i <= 8; i++) {
      const y = cs.priceToY(cs.minPrice + i * priceStep)
      addLine(0, y, cs.width - cs.pr, y, GRID_COLOR)
    }

    const barStep = Math.max(1, Math.floor(100 / cs.barStep))
    for (let i = 0; i < cs.barCount; i += barStep) {
      const x = cs.barToX(i)
      addLine(x, cs.pt, x, cs.height - cs.pb, GRID_COLOR)
    }

    addLine(0, cs.pt, 0, cs.height - cs.pb, AXIS_COLOR)
    addLine(0, cs.height - cs.pb, cs.width - cs.pr, cs.height - cs.pb, AXIS_COLOR)
    addLine(cs.width - cs.pr, cs.pt, cs.width - cs.pr, cs.height - cs.pb, AXIS_COLOR)

    this.vertexCount = offset / FLOATS_PER_VERT
    this.device.queue.writeBuffer(this.lineBuffer, 0, buf, 0, offset)
  }

  render(pass: GPURenderPassEncoder) {
    if (this.vertexCount === 0) return
    pass.setPipeline(this.pipeline)
    pass.setVertexBuffer(0, this.lineBuffer)
    pass.draw(this.vertexCount)
  }

  destroy() { this.lineBuffer.destroy() }
}
