import type { ColumnStore } from '../data/columns'

/** 6 f32s per bar: open, high, low, close, volume, _pad */
const FLOATS_PER_BAR = 6
const BYTES_PER_BAR  = FLOATS_PER_BAR * 4

/**
 * GPU-resident raw OHLCV storage buffer.
 * Data lives on GPU permanently — only 24 bytes are written on tick update,
 * and only the viewport uniform (80 bytes) changes on pan/zoom.
 */
export class GpuBarBuffer {
  private device: GPUDevice
  private gpuBuf: GPUBuffer | null = null
  private capBars = 0
  /** Number of bars currently written to the GPU buffer */
  lenBars = 0
  /** CPU mirror — kept for resize reloads only */
  private cpuMirror: Float32Array = new Float32Array(0)

  constructor(device: GPUDevice) {
    this.device = device
  }

  /** Full load — used on initial load, symbol/timeframe change, or pagination */
  load(data: ColumnStore): void {
    const n = data.length
    if (n === 0) { this.lenBars = 0; return }
    this.ensureCpu(n)
    this.fillCpu(data, 0, n)
    this.ensureGpu(n)
    this.device.queue.writeBuffer(this.gpuBuf!, 0, this.cpuMirror, 0, n * FLOATS_PER_BAR)
    this.lenBars = n
  }

  /** Update just the last bar in-place (live tick — writes 24 bytes to GPU) */
  updateLastBar(data: ColumnStore): void {
    const i = data.length - 1
    if (i < 0) return
    this.ensureCpu(data.length)
    const b = i * FLOATS_PER_BAR
    this.cpuMirror[b]   = data.opens[i]
    this.cpuMirror[b+1] = data.highs[i]
    this.cpuMirror[b+2] = data.lows[i]
    this.cpuMirror[b+3] = data.closes[i]
    this.cpuMirror[b+4] = data.volumes[i]
    this.cpuMirror[b+5] = 0
    if (!this.gpuBuf) {
      // Buffer was never allocated (data was empty on first load) — full upload
      this.ensureGpu(data.length)
      this.device.queue.writeBuffer(this.gpuBuf!, 0, this.cpuMirror, 0, data.length * FLOATS_PER_BAR)
    } else {
      this.device.queue.writeBuffer(this.gpuBuf!, b * 4, this.cpuMirror, b, FLOATS_PER_BAR)
    }
    this.lenBars = data.length
  }

  /** Append a single new bar at the end (new candle) — writes 24 bytes to GPU */
  appendBar(data: ColumnStore, idx: number): void {
    if (idx >= data.length) return
    this.ensureCpu(idx + 1)
    const b = idx * FLOATS_PER_BAR
    this.cpuMirror[b]   = data.opens[idx]
    this.cpuMirror[b+1] = data.highs[idx]
    this.cpuMirror[b+2] = data.lows[idx]
    this.cpuMirror[b+3] = data.closes[idx]
    this.cpuMirror[b+4] = data.volumes[idx]
    this.cpuMirror[b+5] = 0
    // GPU buffer may need to grow (or may not exist yet if data was empty on first load)
    if (!this.gpuBuf || idx >= this.capBars) {
      this.ensureGpu(data.length)
      this.device.queue.writeBuffer(this.gpuBuf!, 0, this.cpuMirror, 0, data.length * FLOATS_PER_BAR)
    } else {
      this.device.queue.writeBuffer(this.gpuBuf!, b * 4, this.cpuMirror, b, FLOATS_PER_BAR)
    }
    this.lenBars = data.length
  }

  get buffer(): GPUBuffer { return this.gpuBuf! }
  get valid(): boolean { return this.gpuBuf !== null && this.lenBars > 0 }

  destroy(): void {
    this.gpuBuf?.destroy()
    this.gpuBuf = null
    this.capBars = 0
    this.lenBars = 0
  }

  private fillCpu(data: ColumnStore, from: number, to: number): void {
    for (let i = from; i < to; i++) {
      const b = i * FLOATS_PER_BAR
      this.cpuMirror[b]   = data.opens[i]
      this.cpuMirror[b+1] = data.highs[i]
      this.cpuMirror[b+2] = data.lows[i]
      this.cpuMirror[b+3] = data.closes[i]
      this.cpuMirror[b+4] = data.volumes[i]
      this.cpuMirror[b+5] = 0
    }
  }

  private ensureCpu(minBars: number): void {
    if (this.cpuMirror.length >= minBars * FLOATS_PER_BAR) return
    const newCap = Math.max(minBars * 2, 1024)
    const next = new Float32Array(newCap * FLOATS_PER_BAR)
    next.set(this.cpuMirror)
    this.cpuMirror = next
  }

  private ensureGpu(minBars: number): void {
    if (this.gpuBuf && this.capBars >= minBars) return
    const newCap = Math.max(minBars * 2, 1024)
    if (this.gpuBuf) this.gpuBuf.destroy()
    this.gpuBuf = this.device.createBuffer({
      size: newCap * BYTES_PER_BAR,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    })
    this.capBars = newCap
  }
}
