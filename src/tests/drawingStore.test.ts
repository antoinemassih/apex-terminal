import { describe, it, expect, beforeEach } from 'vitest'
import { useDrawingStore } from '../store/drawingStore'

describe('drawingStore', () => {
  beforeEach(() => useDrawingStore.getState().clear())

  it('adds a drawing', () => {
    useDrawingStore.getState().addDrawing({
      id: '1', type: 'trendline', points: [], color: '#fff', symbol: 'AAPL', timeframe: '5m'
    })
    expect(useDrawingStore.getState().drawings).toHaveLength(1)
  })

  it('removes a drawing', () => {
    useDrawingStore.getState().addDrawing({
      id: '1', type: 'trendline', points: [], color: '#fff', symbol: 'AAPL', timeframe: '5m'
    })
    useDrawingStore.getState().removeDrawing('1')
    expect(useDrawingStore.getState().drawings).toHaveLength(0)
  })

  it('updates drawing points', () => {
    useDrawingStore.getState().addDrawing({
      id: '1', type: 'trendline', points: [{ time: 0, price: 100 }, { time: 1, price: 200 }],
      color: '#fff', symbol: 'AAPL', timeframe: '5m'
    })
    useDrawingStore.getState().updateDrawing('1', [{ time: 5, price: 150 }, { time: 10, price: 250 }])
    const d = useDrawingStore.getState().drawings[0]
    expect(d.points[0].time).toBe(5)
    expect(d.points[1].price).toBe(250)
  })

  it('toggles draw tool', () => {
    const s = useDrawingStore.getState()
    s.setActiveTool('trendline')
    expect(useDrawingStore.getState().activeTool).toBe('trendline')
    useDrawingStore.getState().toggleDrawTool() // should go back to cursor
    expect(useDrawingStore.getState().activeTool).toBe('cursor')
    useDrawingStore.getState().toggleDrawTool() // should go back to trendline
    expect(useDrawingStore.getState().activeTool).toBe('trendline')
  })

  it('filters by symbol and timeframe', () => {
    const s = useDrawingStore.getState()
    s.addDrawing({ id: '1', type: 'trendline', points: [], color: '#fff', symbol: 'AAPL', timeframe: '5m' })
    s.addDrawing({ id: '2', type: 'trendline', points: [], color: '#fff', symbol: 'MSFT', timeframe: '5m' })
    expect(s.drawingsFor('AAPL', '5m')).toHaveLength(1)
  })
})
