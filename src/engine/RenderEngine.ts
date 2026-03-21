import { FrameScheduler } from './FrameScheduler'
import { PaneContext } from './PaneContext'
import type { GPUContext, EngineState } from './types'

export class RenderEngine {
  private ctx: GPUContext
  private panes = new Map<string, PaneContext>()
  readonly scheduler: FrameScheduler
  private _state: EngineState = 'ready'
  private recoveryAttempts = 0
  private stateListeners = new Set<(state: EngineState) => void>()
  private deviceReplacedListeners = new Set<(device: GPUDevice) => void>()

  private constructor(ctx: GPUContext) {
    this.ctx = ctx
    this.scheduler = new FrameScheduler(ctx.device)
    ctx.device.lost.then(info => this.onDeviceLost(info))
  }

  static async create(): Promise<RenderEngine> {
    const ctx = await RenderEngine.initGPU()
    return new RenderEngine(ctx)
  }

  private static async initGPU(): Promise<GPUContext> {
    if (!navigator.gpu) throw new Error('WebGPU not supported')
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' })
    if (!adapter) throw new Error('No GPU adapter found')
    const device = await adapter.requestDevice()
    return { device, format: navigator.gpu.getPreferredCanvasFormat() }
  }

  get state(): EngineState { return this._state }
  get gpuDevice(): GPUDevice { return this.ctx.device }

  registerPane(id: string, canvas: HTMLCanvasElement): PaneContext {
    const pane = new PaneContext(id, canvas, this.ctx, () => this.scheduler.markDirty(id))
    this.panes.set(id, pane)
    this.scheduler.addPane(pane)
    return pane
  }

  unregisterPane(id: string): void {
    const pane = this.panes.get(id)
    if (pane) {
      pane.destroy()
      this.panes.delete(id)
      this.scheduler.removePane(id)
    }
  }

  retry(): void {
    if (this._state === 'failed') {
      this.recoveryAttempts = 0
      this.recover()
    }
  }

  onStateChange(cb: (state: EngineState) => void): () => void {
    this.stateListeners.add(cb)
    return () => { this.stateListeners.delete(cb) }
  }

  onDeviceReplaced(cb: (device: GPUDevice) => void): () => void {
    this.deviceReplacedListeners.add(cb)
    return () => { this.deviceReplacedListeners.delete(cb) }
  }

  getAllPanes(): IterableIterator<PaneContext> { return this.panes.values() }

  destroy(): void {
    this.scheduler.stop()
    for (const pane of this.panes.values()) pane.destroy()
    this.panes.clear()
  }

  private onDeviceLost(info: GPUDeviceLostInfo): void {
    console.error('GPU device lost:', info.message)
    this.setState('recovering')
    this.scheduler.pause()
    this.recover()
  }

  private async recover(): Promise<void> {
    const MAX_ATTEMPTS = 3
    while (this.recoveryAttempts < MAX_ATTEMPTS) {
      this.recoveryAttempts++
      const delay = 1000 * Math.pow(2, this.recoveryAttempts - 1)
      await new Promise(r => setTimeout(r, delay))

      try {
        this.ctx = await RenderEngine.initGPU()
        this.ctx.device.lost.then(info => this.onDeviceLost(info))
        this.scheduler.updateDevice(this.ctx.device)

        const deadPanes: string[] = []
        for (const [id, pane] of this.panes) {
          if (!pane.canvas.isConnected) {
            deadPanes.push(id)
            continue
          }
          pane.reconfigure(this.ctx)
        }
        for (const id of deadPanes) this.unregisterPane(id)

        for (const cb of this.deviceReplacedListeners) cb(this.ctx.device)

        this.recoveryAttempts = 0
        this.scheduler.resume()
        this.setState('ready')
        return
      } catch (e) {
        console.error(`GPU recovery attempt ${this.recoveryAttempts} failed:`, e)
      }
    }

    this.setState('failed')
  }

  private setState(state: EngineState): void {
    this._state = state
    for (const cb of this.stateListeners) cb(state)
  }
}
