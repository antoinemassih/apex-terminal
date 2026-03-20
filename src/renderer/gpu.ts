export interface GPUContext {
  device: GPUDevice
  format: GPUTextureFormat
}

let _ctx: GPUContext | null = null

export async function getGPUContext(): Promise<GPUContext> {
  if (_ctx) return _ctx
  if (!navigator.gpu) throw new Error('WebGPU not supported')
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' })
  if (!adapter) throw new Error('No GPU adapter found')
  const device = await adapter.requestDevice()
  device.lost.then(info => { console.error('GPU device lost:', info); _ctx = null })
  _ctx = { device, format: navigator.gpu.getPreferredCanvasFormat() }
  return _ctx
}

export function configureCanvas(canvas: HTMLCanvasElement, ctx: GPUContext): GPUCanvasContext {
  const gpuCtx = canvas.getContext('webgpu') as GPUCanvasContext
  gpuCtx.configure({ device: ctx.device, format: ctx.format, alphaMode: 'premultiplied' })
  return gpuCtx
}
