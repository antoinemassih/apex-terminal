import './global.css'
import { Toolbar } from './toolbar/Toolbar'
import { Workspace } from './workspace/Workspace'
import { FrameStats } from './chart/FrameStats'

export default function App() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', width: '100vw', height: '100vh', overflow: 'hidden' }}>
      <Toolbar />
      <div style={{ flex: 1, overflow: 'hidden' }}>
        <Workspace />
      </div>
      <FrameStats />
    </div>
  )
}
