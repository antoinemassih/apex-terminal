const MAX_OUTPUT = 50_000
const EVICT_RATIO = 0.25

export class IncrementalBollinger {
  private buffer: Float64Array
  private pos: number = 0
  private count: number = 0
  private mean: number = 0
  private m2: number = 0
  private sum: number = 0
  private upperOut: Float64Array
  private lowerOut: Float64Array
  private outputLen: number = 0

  constructor(private period: number, private stdDevs: number, capacity: number) {
    this.buffer = new Float64Array(period)
    this.upperOut = new Float64Array(capacity)
    this.lowerOut = new Float64Array(capacity)
  }

  bootstrap(closes: Float64Array, length: number): void {
    this.pos = 0
    this.count = 0
    this.mean = 0
    this.m2 = 0
    this.sum = 0
    this.ensureCapacity(length)
    for (let i = 0; i < length; i++) {
      this.pushInternal(closes[i], i)
    }
    this.outputLen = length
  }

  push(value: number): void {
    if (this.outputLen >= MAX_OUTPUT) {
      this.evict(Math.ceil(this.outputLen * EVICT_RATIO))
    }
    this.ensureCapacity(this.outputLen + 1)
    this.pushInternal(value, this.outputLen)
    this.outputLen++
  }

  updateLast(value: number): void {
    if (this.outputLen === 0) return
    const idx = this.outputLen - 1
    // Find the position of the last pushed value in the circular buffer
    const prevPos = (this.pos - 1 + this.period) % this.period
    const oldValue = this.buffer[prevPos]

    if (oldValue === value) {
      // No change — skip computation
      return
    }

    // O(1) Welford update: remove old value's contribution, add new value's
    if (this.count >= this.period) {
      // Window is full — both old and new values are in the window
      // We're replacing oldValue (already in buffer) with value
      const oldMean = this.mean
      this.sum -= oldValue
      this.sum += value
      this.mean = this.sum / this.period
      // Welford remove-and-add in one step
      this.m2 += (value - oldMean) * (value - this.mean) - (oldValue - oldMean) * (oldValue - this.mean)
      if (this.m2 < 0) this.m2 = 0 // numerical guard
    } else {
      // Window not full — simpler update
      const n = Math.min(this.count, this.period)
      this.sum -= oldValue
      this.sum += value
      this.mean = this.sum / n
      // Recompute m2 from scratch for partial windows (rare, only during bootstrap)
      this.m2 = 0
      this.buffer[prevPos] = value
      for (let i = 0; i < n; i++) {
        const d = this.buffer[i] - this.mean
        this.m2 += d * d
      }
      // Output
      const variance = n >= this.period ? this.m2 / this.period : 0
      const std = Math.sqrt(Math.max(0, variance))
      this.upperOut[idx] = this.count >= this.period ? this.mean + this.stdDevs * std : NaN
      this.lowerOut[idx] = this.count >= this.period ? this.mean - this.stdDevs * std : NaN
      return
    }

    this.buffer[prevPos] = value

    const sma = this.sum / this.period
    const variance = this.m2 / this.period
    const std = Math.sqrt(Math.max(0, variance))
    this.upperOut[idx] = sma + this.stdDevs * std
    this.lowerOut[idx] = sma - this.stdDevs * std
  }

  evict(count: number): void {
    if (count <= 0 || count >= this.outputLen) return
    const keep = this.outputLen - count
    this.upperOut.copyWithin(0, count, this.outputLen)
    this.lowerOut.copyWithin(0, count, this.outputLen)
    this.outputLen = keep
  }

  getUpper(): Float64Array { return this.upperOut }
  getLower(): Float64Array { return this.lowerOut }
  getLength(): number { return this.outputLen }

  private pushInternal(value: number, outputIdx: number): void {
    if (this.count >= this.period) {
      const oldValue = this.buffer[this.pos]
      const oldMean = this.mean
      this.sum -= oldValue
      this.sum += value
      this.mean = this.sum / this.period
      this.m2 += (value - oldMean) * (value - this.mean) - (oldValue - oldMean) * (oldValue - this.mean)
      if (this.m2 < 0) this.m2 = 0
    } else {
      this.sum += value
      const n = this.count + 1
      const oldMean = this.mean
      this.mean = this.sum / n
      this.m2 += (value - oldMean) * (value - this.mean)
    }

    this.buffer[this.pos] = value
    this.pos = (this.pos + 1) % this.period
    this.count++

    const n = Math.min(this.count, this.period)
    const sma = this.sum / n
    const variance = n >= this.period ? this.m2 / this.period : 0
    const std = Math.sqrt(Math.max(0, variance))
    this.upperOut[outputIdx] = this.count >= this.period ? sma + this.stdDevs * std : NaN
    this.lowerOut[outputIdx] = this.count >= this.period ? sma - this.stdDevs * std : NaN
  }

  private ensureCapacity(needed: number): void {
    if (needed <= this.upperOut.length) return
    const newCap = Math.max(needed, this.upperOut.length * 2)
    const newUpper = new Float64Array(newCap)
    const newLower = new Float64Array(newCap)
    newUpper.set(this.upperOut.subarray(0, this.outputLen))
    newLower.set(this.lowerOut.subarray(0, this.outputLen))
    this.upperOut = newUpper
    this.lowerOut = newLower
  }
}
