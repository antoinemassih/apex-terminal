export interface Bar {
  time: number
  open: number
  high: number
  low: number
  close: number
  volume: number
}

export type Timeframe = '1m' | '5m' | '15m' | '30m' | '1h' | '4h' | '1d' | '1wk'
export type DrawingTool = 'cursor' | 'trendline' | 'hline'
export interface Point { time: number; price: number }
export interface Drawing {
  id: string
  type: DrawingTool
  points: Point[]
  color: string
  opacity: number           // 0-1, default 1
  lineStyle: 'solid' | 'dashed' | 'dotted'  // default 'solid'
  thickness: number         // px, default 1.5
  symbol: string
  timeframe: Timeframe
}
