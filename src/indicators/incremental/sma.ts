export class IncrementalSMA {
  private buffer: Float64Array
  private sum: number = 0
  private pos: number = 0
  private count: number = 0
  private output: Float64Array
  private outputLen: number = 0

  constructor(private period: number, capacity: number) {
    this.buffer = new Float64Array(period)
    this.output = new Float64Array(capacity)
  }

  bootstrap(closes: Float64Array, length: number): void {
    this.sum = 0
    this.pos = 0
    this.count = 0
    this.ensureCapacity(length)
    for (let i = 0; i < length; i++) {
      this.pushInternal(closes[i], i)
    }
    this.outputLen = length
  }

  push(value: number): void {
    this.ensureCapacity(this.outputLen + 1)
    this.pushInternal(value, this.outputLen)
    this.outputLen++
  }

  updateLast(value: number): void {
    if (this.outputLen === 0) return
    const idx = this.outputLen - 1
    const prevPos = (this.pos - 1 + this.period) % this.period
    const oldValue = this.buffer[prevPos]
    this.sum -= oldValue
    this.sum += value
    this.buffer[prevPos] = value
    this.output[idx] = this.count >= this.period ? this.sum / this.period : NaN
  }

  getOutput(): Float64Array { return this.output }
  getLength(): number { return this.outputLen }

  private pushInternal(value: number, outputIdx: number): void {
    if (this.count >= this.period) {
      this.sum -= this.buffer[this.pos]
    }
    this.buffer[this.pos] = value
    this.sum += value
    this.pos = (this.pos + 1) % this.period
    this.count++
    this.output[outputIdx] = this.count >= this.period ? this.sum / this.period : NaN
  }

  private ensureCapacity(needed: number): void {
    if (needed <= this.output.length) return
    const newCap = Math.max(needed, this.output.length * 2)
    const newOut = new Float64Array(newCap)
    newOut.set(this.output.subarray(0, this.outputLen))
    this.output = newOut
  }
}
