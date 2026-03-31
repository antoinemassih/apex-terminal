/**
 * Memory manager — prevents WebView2 memory bloat during long trading sessions.
 *
 * - Periodic forced GC (V8's --expose-gc flag is set in WebView2 launch args)
 * - Monitors JS heap and triggers aggressive cleanup when pressure is high
 * - Logs warnings when memory exceeds thresholds
 */

const GC_INTERVAL_MS = 30_000       // Force GC every 30s
const WARN_HEAP_MB = 512            // Log warning above this
const CRITICAL_HEAP_MB = 1024       // Aggressive cleanup above this

let gcTimer: ReturnType<typeof setInterval> | null = null
let monitorTimer: ReturnType<typeof setInterval> | null = null

/** Start the memory manager. Call once after bootstrap. */
export function startMemoryManager(cleanup?: () => void): void {
  // Periodic forced GC — --expose-gc makes gc() available globally
  const gc = (globalThis as any).gc as (() => void) | undefined
  if (gc) {
    gcTimer = setInterval(() => {
      gc()
    }, GC_INTERVAL_MS)
    console.info('[mem] Periodic GC enabled (every 30s)')
  } else {
    console.info('[mem] gc() not available — --expose-gc not set')
  }

  // Heap monitoring via performance.memory (Chrome/Edge only)
  const perf = (performance as any)
  if (perf.memory) {
    monitorTimer = setInterval(() => {
      const used = perf.memory.usedJSHeapSize / (1024 * 1024)
      const total = perf.memory.totalJSHeapSize / (1024 * 1024)

      if (used > CRITICAL_HEAP_MB) {
        console.warn(`[mem] CRITICAL: JS heap ${used.toFixed(0)}MB / ${total.toFixed(0)}MB — triggering cleanup`)
        if (cleanup) cleanup()
        if (gc) gc()
      } else if (used > WARN_HEAP_MB) {
        console.warn(`[mem] WARNING: JS heap ${used.toFixed(0)}MB / ${total.toFixed(0)}MB`)
      }
    }, 10_000) // check every 10s
  }
}

/** Stop the memory manager. */
export function stopMemoryManager(): void {
  if (gcTimer) { clearInterval(gcTimer); gcTimer = null }
  if (monitorTimer) { clearInterval(monitorTimer); monitorTimer = null }
}
