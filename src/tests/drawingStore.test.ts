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

  it('filters by symbol and timeframe', () => {
    const s = useDrawingStore.getState()
    s.addDrawing({ id: '1', type: 'trendline', points: [], color: '#fff', symbol: 'AAPL', timeframe: '5m' })
    s.addDrawing({ id: '2', type: 'trendline', points: [], color: '#fff', symbol: 'MSFT', timeframe: '5m' })
    expect(s.drawingsFor('AAPL', '5m')).toHaveLength(1)
  })
})
