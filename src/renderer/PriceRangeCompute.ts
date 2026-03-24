import type { GPUContext } from '../engine/types'
import type { GpuBarBuffer } from './GpuBarBuffer'
import shaderSrc from './shaders/min_max_reduce.wgsl?raw'

const RESULT_BYTES  = 8   // 2 × u32
const UNIFORM_BYTES = 16  // viewStart, viewCount, _pad×2

/**
 * Async GPU min/max price range computation.
 *
 * Each frame:
 *   1. dispatch(encoder, gpuBars, viewStart, viewCount) — adds compute pass to encoder
 *   2. After device.queue.submit(), call postSubmit() — fires async GPU→CPU readback
 *   3. Next frame: priceRange returns the result (1-frame delay)
 *
 * Falls back to null until the first result arrives (CPU fallback in caller).
 */
export class PriceRangeCompute {
  private readonly pipeline: GPUComputePipeline
  private readonly bgl: GPUBindGroupLayout
  private readonly resultBuf: GPUBuffer   // STORAGE | COPY_SRC
  private readonly stagingBuf: GPUBuffer  // MAP_READ | COPY_DST
  private readonly uniformBuf: GPUBuffer  // UNIFORM | COPY_DST
  private bindGroup: GPUBindGroup | null = null
  private lastStorageBuf: GPUBuffer | null = null
  private readonly device: GPUDevice
  private pendingMap = false
  private _result: { min: number; max: number } | null = null
  private readonly clearData  = new Uint32Array([0xFFFFFFFF, 0x00000000])
  private readonly vpU32      = new Uint32Array(4)
  private readonly readFloats = new Float32Array(2)
  private readonly readU32    = new Uint32Array(this.readFloats.buffer)

  constructor(ctx: GPUContext) {
    this.device = ctx.device

    const module = ctx.device.createShaderModule({ code: shaderSrc })

    this.bgl = ctx.device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'read-only-storage' } },
        { binding: 1, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'uniform' } },
        { binding: 2, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
      ],
    })

    this.pipeline = ctx.device.createComputePipeline({
      layout: ctx.device.createPipelineLayout({ bindGroupLayouts: [this.bgl] }),
      compute: { module, entryPoint: 'cs_main' },
    })

    this.resultBuf = ctx.device.createBuffer({
      size: RESULT_BYTES,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
    })

    this.stagingBuf = ctx.device.createBuffer({
      size: RESULT_BYTES,
      usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
    })

    this.uniformBuf = ctx.device.createBuffer({
      size: UNIFORM_BYTES,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    })
  }

  /** Last computed price range — null until first result (use CPU fallback). */
  get priceRange(): { min: number; max: number } | null {
    return this._result
  }

  /**
   * Encode compute dispatch + result copy into the command encoder.
   * Must be called before the render pass in the same encoder.
   */
  dispatch(
    encoder: GPUCommandEncoder,
    gpuBars: GpuBarBuffer,
    viewStart: number,
    viewCount: number,
  ): void {
    // Skip if staging buffer has a pending mapAsync — can't copy into a mapped buffer
    if (!gpuBars.valid || viewCount <= 0 || this.pendingMap) return

    // Rebuild bind group only when the bars buffer was reallocated
    if (gpuBars.buffer !== this.lastStorageBuf) {
      this.bindGroup = this.device.createBindGroup({
        layout: this.bgl,
        entries: [
          { binding: 0, resource: { buffer: gpuBars.buffer } },
          { binding: 1, resource: { buffer: this.uniformBuf } },
          { binding: 2, resource: { buffer: this.resultBuf } },
        ],
      })
      this.lastStorageBuf = gpuBars.buffer
    }

    // Clamp to actual written bars — uninitialized GPU slots beyond lenBars contain zeros,
    // which would be read as LOW=0, collapsing the price range to 0→max and squishing candles.
    const safeCount = Math.min(viewCount, Math.max(0, gpuBars.lenBars - viewStart))
    if (safeCount <= 0) return

    // Write dispatch params
    this.vpU32[0] = viewStart; this.vpU32[1] = safeCount; this.vpU32[2] = 0; this.vpU32[3] = 0
    this.device.queue.writeBuffer(this.uniformBuf, 0, this.vpU32)

    // Clear result (min = MAX_U32, max = 0) before dispatch
    this.device.queue.writeBuffer(this.resultBuf, 0, this.clearData)

    const pass = encoder.beginComputePass()
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.bindGroup!)
    pass.dispatchWorkgroups(Math.ceil(safeCount / 256))
    pass.end()

    // Copy result to CPU-readable staging buffer
    encoder.copyBufferToBuffer(this.resultBuf, 0, this.stagingBuf, 0, RESULT_BYTES)
  }

  /**
   * Call after device.queue.submit().
   * Fires async GPU→CPU readback; result is available next frame via .priceRange.
   */
  postSubmit(): void {
    if (this.pendingMap) return
    this.pendingMap = true
    this.stagingBuf.mapAsync(GPUMapMode.READ).then(() => {
      const view = new Uint32Array(this.stagingBuf.getMappedRange())
      const minU = view[0]
      const maxU = view[1]
      this.stagingBuf.unmap()
      this.pendingMap = false

      // Sentinel check: all-bits-set means no bars were processed
      if (minU === 0xFFFFFFFF || maxU === 0) { this._result = null; return }

      this.readU32[0] = minU
      this.readU32[1] = maxU
      const minF = this.readFloats[0]
      const maxF = this.readFloats[1]

      if (!isFinite(minF) || !isFinite(maxF) || minF > maxF) { this._result = null; return }
      this._result = { min: minF, max: maxF }
    }).catch(() => { this.pendingMap = false })
  }

  destroy(): void {
    this.resultBuf.destroy()
    this.stagingBuf.destroy()
    this.uniformBuf.destroy()
    this.bindGroup      = null
    this.lastStorageBuf = null
    this._result        = null
  }
}
