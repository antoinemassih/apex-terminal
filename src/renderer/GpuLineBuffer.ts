/** 4 bytes per value — f32 storage for indicator line data */
const BYTES_PER_FLOAT = 4

/**
 * GPU-resident indicator value storage buffer.
 * Mirrors GpuBarBuffer but stores a single f32 per bar (one indicator line).
 *
 * Float64Array → Float32Array conversion on upload is intentional:
 * stock/indicator values have at most 7 significant digits — well within f32.
 */
export class GpuLineBuffer {
  private device: GPUDevice
  private gpuBuf: GPUBuffer | null = null
  private capFloats = 0
  /** Number of values currently written to the GPU buffer */
  lenFloats = 0
  /** CPU mirror — kept in Float32Array for fast writeBuffer calls */
  private cpuMirror: Float32Array = new Float32Array(0)

  constructor(device: GPUDevice) {
    this.device = device
  }

  /** Full upload — used on symbol/TF change, indicator reconfiguration */
  load(values: Float64Array): void {
    const n = values.length
    if (n === 0) { this.lenFloats = 0; return }
    this.ensureCpu(n)
    // Convert f64 → f32; NaN is preserved (checked in shader via bitcast)
    for (let i = 0; i < n; i++) this.cpuMirror[i] = values[i]
    this.ensureGpu(n)
    this.device.queue.writeBuffer(this.gpuBuf!, 0, this.cpuMirror, 0, n)
    this.lenFloats = n
  }

  /** Update last value in-place (tick path) — writes 4 bytes to GPU */
  updateLast(values: Float64Array): void {
    const i = values.length - 1
    if (i < 0 || !this.gpuBuf) return
    this.ensureCpu(values.length)
    this.cpuMirror[i] = values[i]
    this.device.queue.writeBuffer(this.gpuBuf!, i * BYTES_PER_FLOAT, this.cpuMirror, i, 1)
    this.lenFloats = values.length
  }

  /** Append one new value (new bar path) — writes 4 bytes, or full reload if buffer grew */
  appendValue(values: Float64Array): void {
    const i = values.length - 1
    if (i < 0) return
    this.ensureCpu(values.length)
    this.cpuMirror[i] = values[i]
    if (i >= this.capFloats) {
      this.ensureGpu(values.length)
      this.device.queue.writeBuffer(this.gpuBuf!, 0, this.cpuMirror, 0, values.length)
    } else {
      if (!this.gpuBuf) return
      this.device.queue.writeBuffer(this.gpuBuf!, i * BYTES_PER_FLOAT, this.cpuMirror, i, 1)
    }
    this.lenFloats = values.length
  }

  get buffer(): GPUBuffer { return this.gpuBuf! }
  get valid(): boolean { return this.gpuBuf !== null && this.lenFloats > 0 }

  destroy(): void {
    this.gpuBuf?.destroy()
    this.gpuBuf = null
    this.capFloats = 0
    this.lenFloats = 0
  }

  private ensureCpu(minFloats: number): void {
    if (this.cpuMirror.length >= minFloats) return
    const newCap = Math.max(minFloats * 2, 256)
    const next = new Float32Array(newCap)
    next.set(this.cpuMirror)
    this.cpuMirror = next
  }

  private ensureGpu(minFloats: number): void {
    if (this.gpuBuf && this.capFloats >= minFloats) return
    const newCap = Math.max(minFloats * 2, 256)
    if (this.gpuBuf) this.gpuBuf.destroy()
    this.gpuBuf = this.device.createBuffer({
      size: newCap * BYTES_PER_FLOAT,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    })
    this.capFloats = newCap
  }
}
