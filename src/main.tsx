import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { RenderEngine } from './engine'
import { IndicatorEngine } from './indicators'
import { DataStore, SimulatedFeed } from './data'
import { setRenderEngine, setDataStore, setIndicatorEngine, setFeed } from './globals'
import { useChartStore } from './store/chartStore'

async function bootstrap() {
  const engine = await RenderEngine.create()
  const indicatorEngine = new IndicatorEngine()
  const dataStore = new DataStore(indicatorEngine)
  const feed = new SimulatedFeed()

  feed.onTick((symbol, tf, tick) => dataStore.applyTick(symbol, tf, tick))

  // Subscribe all default panes to the feed
  const panes = useChartStore.getState().panes
  for (const pane of panes) {
    feed.subscribe(pane.symbol, pane.timeframe)
  }

  await feed.connect()

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
