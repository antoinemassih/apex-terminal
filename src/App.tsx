import './global.css'
import { Toolbar } from './toolbar/Toolbar'
import { Workspace } from './workspace/Workspace'
import { Watchlist } from './watchlist/Watchlist'
import { OrdersPanel } from './orders/OrdersPanel'

export default function App() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', width: '100vw', height: '100vh', overflow: 'hidden' }}>
      <Toolbar />
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        <Workspace />
        <OrdersPanel />
        <Watchlist />
      </div>
    </div>
  )
}
