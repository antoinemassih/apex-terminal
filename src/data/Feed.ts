import type { TickData } from './types'

export interface Feed {
  connect(): Promise<void>
  disconnect(): void
  subscribe(symbol: string, timeframe: string): void
  unsubscribe(symbol: string, timeframe: string): void
  onTick(cb: (symbol: string, timeframe: string, tick: TickData) => void): () => void
  onDisconnect(cb: () => void): () => void
}
