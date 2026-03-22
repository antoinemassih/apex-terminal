export { CoordSystem, type CoordConfig } from '../chart/CoordSystem'

export interface GPUContext {
  device: GPUDevice
  format: GPUTextureFormat
}

export type EngineState = 'uninitialized' | 'ready' | 'recovering' | 'failed'

export interface FrameStats {
  /** Actual renders per second (data update rate — not 60fps target) */
  updatesPerSec: number
  /** Time spent rendering the last frame (GPU command recording + submit) */
  renderTimeMs: number
  /** Peak render time in recent history */
  renderTimePeak: number
  /** Number of panes rendered in last frame */
  panesRendered: number
  /** Total panes registered */
  panesTotal: number
}
