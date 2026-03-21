export { CoordSystem, type CoordConfig } from '../chart/CoordSystem'

export interface GPUContext {
  device: GPUDevice
  format: GPUTextureFormat
}

export type EngineState = 'uninitialized' | 'ready' | 'recovering' | 'failed'
