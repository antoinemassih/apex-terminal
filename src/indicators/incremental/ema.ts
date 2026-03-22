export class IncrementalEMA {
  private k: number
  private prevEma: number = 0
  private output: Float64Array
  private outputLen: number = 0
  private count: number = 0

  constructor(private period: number, capacity: number) {
    this.k = 2 / (period + 1)
    this.output = new Float64Array(capacity)
  }

  bootstrap(closes: Float64Array, length: number): void {
    this.count = 0
    this.prevEma = 0
    this.ensureCapacity(length)
    if (length === 0) { this.outputLen = 0; return }
    this.prevEma = closes[0]
    this.output[0] = NaN
    this.count = 1
    for (let i = 1; i < length; i++) {
      this.prevEma = closes[i] * this.k + this.prevEma * (1 - this.k)
      this.output[i] = i >= this.period - 1 ? this.prevEma : NaN
      this.count++
    }
    this.outputLen = length
  }

  push(value: number): void {
    this.ensureCapacity(this.outputLen + 1)
    this.prevEma = value * this.k + this.prevEma * (1 - this.k)
    this.output[this.outputLen] = this.count >= this.period - 1 ? this.prevEma : NaN
    this.outputLen++
    this.count++
  }

  updateLast(value: number): void {
    if (this.outputLen < 2) return
    const prevIdx = this.outputLen - 2
    const prevEmaVal = !isNaN(this.output[prevIdx]) ? this.output[prevIdx] : this.prevEma
    this.prevEma = value * this.k + prevEmaVal * (1 - this.k)
    // Use same threshold as push: count >= period - 1
    this.output[this.outputLen - 1] = this.count >= this.period - 1 ? this.prevEma : NaN
  }

  getOutput(): Float64Array { return this.output }
  getLength(): number { return this.outputLen }

  private ensureCapacity(needed: number): void {
    if (needed <= this.output.length) return
    const newCap = Math.max(needed, this.output.length * 2)
    const newOut = new Float64Array(newCap)
    newOut.set(this.output.subarray(0, this.outputLen))
    this.output = newOut
  }
}
