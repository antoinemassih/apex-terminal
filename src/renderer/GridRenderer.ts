import { GPUContext } from './gpu'
import { CoordSystem } from '../chart/CoordSystem'
import shaderSrc from './shaders/grid.wgsl?raw'

const GRID_COLOR = [0.15, 0.15, 0.15, 1.0]
const AXIS_COLOR = [0.3, 0.3, 0.3, 1.0]

export class GridRenderer {
  private pipeline: GPURenderPipeline
  private lineBuffer: GPUBuffer | null = null
  private vertexCount = 0
  private readonly device: GPUDevice

  constructor(ctx: GPUContext) {
    this.device = ctx.device
    const module = ctx.device.createShaderModule({ code: shaderSrc })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: 'auto',
      vertex: {
        module, entryPoint: 'vs_main',
        buffers: [{
          arrayStride: 6 * 4,
          attributes: [
            { shaderLocation: 0, offset: 0, format: 'float32x2' },
            { shaderLocation: 1, offset: 8, format: 'float32x4' },
          ],
        }],
      },
      fragment: { module, entryPoint: 'fs_main', targets: [{ format: ctx.format }] },
      primitive: { topology: 'line-list' },
    })
  }

  upload(cs: CoordSystem) {
    const verts: number[] = []
    const addLine = (x0: number, y0: number, x1: number, y1: number, color: number[]) => {
      const cx0 = (x0 / cs.width) * 2 - 1, cy0 = 1 - (y0 / cs.height) * 2
      const cx1 = (x1 / cs.width) * 2 - 1, cy1 = 1 - (y1 / cs.height) * 2
      verts.push(cx0, cy0, ...color, cx1, cy1, ...color)
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

    const data = new Float32Array(verts)
    if (this.lineBuffer) this.lineBuffer.destroy()
    this.lineBuffer = this.device.createBuffer({
      size: data.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    })
    this.device.queue.writeBuffer(this.lineBuffer, 0, data)
    this.vertexCount = verts.length / 6
  }

  render(pass: GPURenderPassEncoder) {
    if (!this.lineBuffer || this.vertexCount === 0) return
    pass.setPipeline(this.pipeline)
    pass.setVertexBuffer(0, this.lineBuffer)
    pass.draw(this.vertexCount)
  }

  destroy() { this.lineBuffer?.destroy() }
}
