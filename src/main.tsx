import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { RenderEngine } from './engine'
import { IndicatorEngine } from './indicators'
import { DataStore, SimulatedFeed, BarCache } from './data'
import { LocalDrawingRepository } from './data/DrawingRepository'
import { TauriDrawingRepository } from './data/TauriDrawingRepository'
import { OcocoClient } from './data/OcocoClient'
import { setRenderEngine, setDataStore, setIndicatorEngine, setFeed } from './globals'
import { useChartStore } from './store/chartStore'
import { initDrawingStore } from './store/drawingStore'

async function bootstrap() {
  const engine = await RenderEngine.create()
  const indicatorEngine = new IndicatorEngine()

  // Init persistence layers
  const barCache = new BarCache()
  await barCache.init()

  // Drawing persistence: OCOCO API → Tauri IPC → IndexedDB (fallback chain)
  const OCOCO_API = 'http://192.168.1.60:30300'
  let drawingRepo: OcocoClient | TauriDrawingRepository | LocalDrawingRepository
  // Store OCOCO client globally for WS signal subscription from chart panes
  ;(window as any).__ococoClient = null as OcocoClient | null

  try {
    const client = new OcocoClient(OCOCO_API)
    await client.loadAll() // test connection
    drawingRepo = client
    ;(window as any).__ococoClient = client
    client.connectWs()
    console.info('Drawings: using OCOCO API')
  } catch {
    try {
      const tauriRepo = new TauriDrawingRepository()
      await tauriRepo.loadAll()
      drawingRepo = tauriRepo
      console.info('Drawings: using Tauri → PostgreSQL')
    } catch {
      const localRepo = new LocalDrawingRepository()
      await localRepo.init()
      await localRepo.migrateFromLocalStorage()
      drawingRepo = localRepo
      console.info('Drawings: using IndexedDB (offline)')
    }
  }
  await initDrawingStore(drawingRepo)

  const dataStore = new DataStore(indicatorEngine, barCache)
  const feed = new SimulatedFeed()

  feed.onTick((symbol, tf, tick) => dataStore.applyTick(symbol, tf, tick))

  // Subscribe all default panes to the feed
  const panes = useChartStore.getState().panes
  for (const pane of panes) {
    feed.subscribe(pane.symbol, pane.timeframe)
  }

  await feed.connect()

  // Feed lifecycle events
  feed.onDisconnect(() => console.warn('Feed disconnected'))
  feed.onReconnect(() => {
    console.info('Feed reconnected')
    for (const pane of engine.getAllPanes()) pane.dirty = true
  })

  setRenderEngine(engine)
  setDataStore(dataStore)
  setIndicatorEngine(indicatorEngine)
  setFeed(feed)

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
