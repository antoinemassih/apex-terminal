import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { RenderEngine } from './engine'
import { IndicatorEngine } from './indicators'
import { DataStore, BarCache } from './data'
import { IBKRProvider } from './data/IBKRProvider'
import { LocalDrawingRepository } from './data/DrawingRepository'
import { TauriDrawingRepository } from './data/TauriDrawingRepository'
import { OcocoClient } from './data/OcocoClient'
import { setRenderEngine, setDataStore, setIndicatorEngine, setDataProvider } from './globals'
import { useChartStore } from './store/chartStore'
import { initDrawingStore } from './store/drawingStore'

// Prevent WebView2/Win32 from processing the contextmenu event at the OS level.
// This must run before any component registers onContextMenu handlers.
document.addEventListener('contextmenu', e => e.preventDefault(), { capture: true })

async function bootstrap() {
  console.info('[boot] 1 GPU init...')
  const engine = await RenderEngine.create()
  console.info('[boot] 2 GPU ready')
  const indicatorEngine = new IndicatorEngine()

  // Init persistence layers
  const barCache = new BarCache()
  console.info('[boot] 3 BarCache init...')
  await barCache.init()
  console.info('[boot] 4 BarCache ready')

  // Drawing persistence: OCOCO API → Tauri IPC → IndexedDB (fallback chain)
  const OCOCO_API = 'http://192.168.1.60:30300'
  let drawingRepo: OcocoClient | TauriDrawingRepository | LocalDrawingRepository
  // Store OCOCO client globally for WS signal subscription from chart panes
  ;(window as any).__ococoClient = null as OcocoClient | null

  console.info('[boot] 5 drawing repo...')
  try {
    const client = new OcocoClient(OCOCO_API)
    await client.loadAll() // test connection
    drawingRepo = client
    ;(window as any).__ococoClient = client
    client.connectWs()
    console.info('[boot] Drawings: using OCOCO API')
  } catch {
    try {
      const tauriRepo = new TauriDrawingRepository()
      await tauriRepo.loadAll()
      drawingRepo = tauriRepo
      console.info('[boot] Drawings: using Tauri → PostgreSQL')
    } catch {
      const localRepo = new LocalDrawingRepository()
      await localRepo.init()
      await localRepo.migrateFromLocalStorage()
      drawingRepo = localRepo
      console.info('[boot] Drawings: using IndexedDB (offline)')
    }
  }
  console.info('[boot] 6 drawing store init...')
  await initDrawingStore(drawingRepo)

  // Data provider (swap implementation here for different data sources)
  const provider = new IBKRProvider()

  const dataStore = new DataStore(indicatorEngine, provider, barCache)

  provider.onTick((symbol, tf, tick) => dataStore.applyTick(symbol, tf, tick))

  // Subscribe all default panes to the provider
  const panes = useChartStore.getState().panes
  for (const pane of panes) {
    provider.subscribe(pane.symbol, pane.timeframe)
  }

  console.info('[boot] 7 provider connect...')
  await provider.connect()
  console.info(`[boot] 8 provider ready: ${provider.name}`)

  // Feed lifecycle events
  provider.onDisconnect(() => console.warn('Data provider disconnected'))
  provider.onReconnect(() => {
    console.info('Data provider reconnected')
    for (const pane of engine.getAllPanes()) pane.dirty = true
  })

  setRenderEngine(engine)
  setDataStore(dataStore)
  setIndicatorEngine(indicatorEngine)
  setDataProvider(provider)

  engine.scheduler.start()

  // Tab visibility handling
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
      engine.scheduler.stop()
    } else {
      for (const pane of engine.getAllPanes()) pane.dirty = true
      engine.scheduler.start()
    }
  })

  console.info('[boot] 9 React mounting...')
  ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  )
}

bootstrap().catch(err => {
  console.error('Bootstrap failed:', err)
  document.body.innerHTML = `<div style="color:#e74c3c;padding:40px;font-family:monospace">
    GPU initialization failed: ${err.message}<br><br>
    <button onclick="location.reload()">Retry</button>
  </div>`
})
