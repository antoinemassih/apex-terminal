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
import { startMemoryManager } from './memoryManager'

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
  // Clean up previous client on re-bootstrap (hot reload / StrictMode)
  if ((window as any).__ococoClient) {
    try { (window as any).__ococoClient.disconnectWs() } catch {}
  }
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

  provider.onTick((symbol, tf, tick) => {
    dataStore.applyTick(symbol, tf, tick)
    // Forward ticks to native GPU chart (fire-and-forget)
    if ((window as any).__nativeChartInvoke) {
      (window as any).__nativeChartInvoke('native_chart_tick', {
        symbol, price: tick.price, volume: tick.volume,
      }).catch(() => {})
    }
  })

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

  // Memory management — periodic GC + pressure monitoring
  startMemoryManager(() => dataStore.evictAll())

  // Native GPU chart ↔ WebView bridge: when native chart switches symbol,
  // load data via DataStore and send it back to Rust.
  // Also forward live ticks to native chart via invoke.
  try {
    const { listen } = await import('@tauri-apps/api/event')
    const { invoke } = await import('@tauri-apps/api/core')
    // Expose invoke for tick forwarding (set by onTick callback above)
    ;(window as any).__nativeChartInvoke = invoke
    listen<{ symbol: string; timeframe: string }>('native-chart-load', async ({ payload }) => {
      const { symbol, timeframe } = payload
      console.info(`[native-chart] WebView loading ${symbol} ${timeframe}`)
      try {
        // Ensure the provider is subscribed so ticks flow
        provider.subscribe(symbol, timeframe)
        // Load data (may fetch from cache or network)
        const { data } = await dataStore.load(symbol, timeframe)
        if (data && data.length > 0) {
          const bars: { open: number; high: number; low: number; close: number; volume: number; time: number }[] = []
          for (let i = 0; i < data.length; i++) {
            bars.push({
              open: data.opens[i], high: data.highs[i], low: data.lows[i],
              close: data.closes[i], volume: data.volumes[i], time: Math.floor(data.times[i]),
            })
          }
          await invoke('native_chart_data', { symbol, timeframe, bars })
          console.info(`[native-chart] Sent ${bars.length} bars for ${symbol}`)
        } else {
          console.warn(`[native-chart] No data available for ${symbol}`)
        }
      } catch (e) {
        console.error(`[native-chart] Failed to load ${symbol}:`, e)
      }
    })
    console.info('[boot] native-chart-load listener registered')
  } catch {
    // Not running in Tauri — skip
  }

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
