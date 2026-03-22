import type { Bar } from '../types'

const DB_NAME = 'apex-bars'
const DB_VERSION = 1
const STORE_NAME = 'bars'

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME)
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

export class BarCache {
  private db: IDBDatabase | null = null

  async init(): Promise<void> {
    try {
      this.db = await openDB()
    } catch (e) {
      console.warn('BarCache: IndexedDB unavailable, caching disabled', e)
    }
  }

  async get(symbol: string, timeframe: string): Promise<Bar[] | null> {
    if (!this.db) return null
    const key = `${symbol}:${timeframe}`
    return new Promise((resolve) => {
      try {
        const tx = this.db!.transaction(STORE_NAME, 'readonly')
        const req = tx.objectStore(STORE_NAME).get(key)
        req.onsuccess = () => resolve(req.result ?? null)
        req.onerror = () => resolve(null)
      } catch {
        resolve(null)
      }
    })
  }

  async set(symbol: string, timeframe: string, bars: Bar[]): Promise<void> {
    if (!this.db) return
    const key = `${symbol}:${timeframe}`
    return new Promise((resolve) => {
      try {
        const tx = this.db!.transaction(STORE_NAME, 'readwrite')
        tx.objectStore(STORE_NAME).put(bars, key)
        tx.oncomplete = () => resolve()
        tx.onerror = () => resolve()
      } catch {
        resolve()
      }
    })
  }
}
