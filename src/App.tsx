import './global.css'
import { ChartPane } from './chart/ChartPane'

export default function App() {
  return (
    <div style={{ width: '100vw', height: '100vh', overflow: 'hidden', background: '#0d0d0d' }}>
      <ChartPane symbol="AAPL" timeframe="5m" width={1200} height={700} />
    </div>
  )
}
