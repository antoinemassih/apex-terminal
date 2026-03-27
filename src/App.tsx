import './global.css'
import { Toolbar } from './toolbar/Toolbar'
import { Workspace } from './workspace/Workspace'
import { Watchlist } from './watchlist/Watchlist'
import { OrdersPanel } from './orders/OrdersPanel'
import { ErrorBoundary } from './ErrorBoundary'

export default function App() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', width: '100vw', height: '100vh', overflow: 'hidden' }}>
      <ErrorBoundary name="Toolbar">
        <Toolbar />
      </ErrorBoundary>
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        <ErrorBoundary name="Charts">
          <Workspace />
        </ErrorBoundary>
        <ErrorBoundary name="Orders">
          <OrdersPanel />
        </ErrorBoundary>
        <ErrorBoundary name="Watchlist">
          <Watchlist />
        </ErrorBoundary>
      </div>
    </div>
  )
}
