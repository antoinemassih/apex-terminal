export interface Bar {
  time: number
  open: number
  high: number
  low: number
  close: number
  volume: number
}

export type Timeframe = '1m' | '5m' | '15m' | '1h' | '4h' | '1d' | '1wk'
export type DrawingTool = 'cursor' | 'trendline' | 'hline'
export interface Point { time: number; price: number }
export interface Drawing {
  id: string
  type: DrawingTool
  points: Point[]
  color: string
  symbol: string
  timeframe: Timeframe
}
