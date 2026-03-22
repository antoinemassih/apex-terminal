import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { RenderEngine } from './engine'
import { IndicatorEngine } from './indicators'
import { DataStore, SimulatedFeed, BarCache } from './data'
import { LocalDrawingRepository } from './data/DrawingRepository'
import { TauriDrawingRepository } from './data/TauriDrawingRepository'
import { setRenderEngine, setDataStore, setIndicatorEngine, setFeed } from './globals'
import { useChartStore } from './store/chartStore'
import { initDrawingStore } from './store/drawingStore'

async function bootstrap() {
  const engine = await RenderEngine.create()
  const indicatorEngine = new IndicatorEngine()

  // Init persistence layers
  const barCache = new BarCache()
  await barCache.init()

  // Use PostgreSQL via Tauri IPC for drawing persistence
  // Falls back to IndexedDB if DB connection fails
  let drawingRepo: LocalDrawingRepository | TauriDrawingRepository
  try {
    const tauriRepo = new TauriDrawingRepository()
    // Test connection by loading (will throw if DB is unreachable)
    await tauriRepo.loadAll()
    drawingRepo = tauriRepo
    console.info('Drawings: using PostgreSQL (ococo)')

    // Migrate any local drawings to DB
    const localRepo = new LocalDrawingRepository()
    await localRepo.init()
    const localDrawings = await localRepo.loadAll()
    if (localDrawings.length > 0) {
      for (const d of localDrawings) await tauriRepo.save(d)
      await localRepo.clear()
      console.info(`Migrated ${localDrawings.length} local drawings to PostgreSQL`)
    }
  } catch (e) {
    console.warn('PostgreSQL unavailable, falling back to IndexedDB:', e)
    const localRepo = new LocalDrawingRepository()
    await localRepo.init()
    await localRepo.migrateFromLocalStorage()
    drawingRepo = localRepo
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
