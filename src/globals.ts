import type { RenderEngine } from './engine'
import type { DataStore } from './data'
import type { IndicatorEngine } from './indicators'
import type { DataProvider } from './data/DataProvider'

let _engine: RenderEngine | null = null
let _dataStore: DataStore | null = null
let _indicatorEngine: IndicatorEngine | null = null
let _provider: DataProvider | null = null

export function getRenderEngine(): RenderEngine {
  if (!_engine) throw new Error('RenderEngine not initialized')
  return _engine
}
export function setRenderEngine(e: RenderEngine) { _engine = e }

export function getDataStore(): DataStore {
  if (!_dataStore) throw new Error('DataStore not initialized')
  return _dataStore
}
export function setDataStore(d: DataStore) { _dataStore = d }

export function getIndicatorEngine(): IndicatorEngine {
  if (!_indicatorEngine) throw new Error('IndicatorEngine not initialized')
  return _indicatorEngine
}
export function setIndicatorEngine(i: IndicatorEngine) { _indicatorEngine = i }

export function getDataProvider(): DataProvider {
  if (!_provider) throw new Error('DataProvider not initialized')
  return _provider
}
export function setDataProvider(p: DataProvider) { _provider = p }
