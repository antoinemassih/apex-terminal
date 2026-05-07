//! Lightweight frame profiler. Tracks per-section CPU time per frame
//! using a thread-local zone stack. Zero allocation in the hot path
//! (uses fixed-size arrays).
//!
//! Usage:
//!   {
//!       let _z = profile_zone("render_chart_pane");
//!       // ... render code ...
//!   }
//!
//! At end of frame, `frame_end()` rolls the zones into a snapshot
//! readable by the perf HUD.
//!
//! Frame pacing note: the renderer is currently egui-driven — egui issues
//! `request_repaint()` from many places (hover, drag, animation, replay,
//! incoming network data). True vsync locking is *not* implemented here:
//! that would require winit-level changes (e.g. throttling redraw requests
//! to monitor refresh rate, or detecting double-paints in a single vblank
//! and warning). The data this profiler exposes is the prerequisite for
//! that work — first measure where time is spent, then optimize / lock.

use std::cell::RefCell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

// ─── Repaint / idle tracking ─────────────────────────────────────────────────
//
// In addition to per-zone CPU time, the profiler tracks two signals that let
// us reason about frame pacing:
//
//   1. RepaintStats — how many times something asked for a redraw this frame,
//      and the file:line of the last requester. >10/frame is a "storm" and
//      gets logged to stderr (rate-limited, once per minute).
//
//   2. is_idle()    — true when the last 30 frames had zero input events and
//      zero in-flight animations. Background workers (drawing-db save,
//      live_state push) can read this to defer/batch their work and avoid
//      keeping the UI thread busy when the user isn't interacting.
//
// These fields are NOT yet rendered by perf_hud.rs (out of scope for this
// pass). When the perf HUD agent extends that file, the relevant accessors are:
//      last_frame_repaints() -> RepaintStats
//      is_idle()             -> bool
//
// `note_repaint(file_line)` should be called immediately before any
// `request_redraw` / `request_repaint` site so the profiler can attribute the
// request. Animation-driven repaints (motion::ease_*) intentionally still
// flow through the same counter — they're not bugs, just useful telemetry.

/// Maximum zones recorded per frame. Anything beyond this is silently dropped
/// to keep the hot path allocation-free.
const MAX_ZONES_PER_FRAME: usize = 256;
/// Number of recent frame snapshots retained for the perf HUD spark line.
const HISTORY_DEPTH: usize = 240; // ~4 seconds at 60fps

#[derive(Clone, Copy, Debug)]
pub struct ZoneSample {
    pub name: &'static str,
    /// Microseconds since the start of the current frame.
    pub start_us: u64,
    pub duration_us: u64,
    /// Nesting depth (0 = top-level).
    pub depth: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FrameSnapshot {
    pub frame_index: u64,
    pub total_us: u64,
    pub zone_count: u16,
}

/// Per-frame repaint-request stats. Rolled into `History` at `frame_end`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RepaintStats {
    pub count: u32,
    /// `file:line` of the most recent caller this frame.
    pub last_caller: Option<&'static str>,
}

/// Frame-local repaint counter. Reset by `frame_begin`, drained by `frame_end`.
/// Atomic so non-render-thread code paths (e.g. async data callbacks marshalled
/// back through egui) can also call `note_repaint` safely without contending
/// on a mutex on the hot path.
static REPAINT_COUNT: AtomicU32 = AtomicU32::new(0);
/// Last `file:line` to request a repaint this frame. `&'static str` so we can
/// store the pointer in a single u64 via `Mutex<Option<&'static str>>` — but
/// since it's only read once per frame, a small mutex is fine.
static LAST_CALLER: Mutex<Option<&'static str>> = Mutex::new(None);

/// Frame-local input-event counter (set by gpu.rs each frame from
/// `ctx.input(|i| i.events.len())`). Used by `is_idle`.
static INPUT_EVENT_COUNT: AtomicU32 = AtomicU32::new(0);
/// Number of in-flight animations. Bumped/decremented by motion helpers.
/// Read by `is_idle` — non-zero ⇒ not idle.
static ANIMATIONS_IN_FLIGHT: AtomicU32 = AtomicU32::new(0);
/// Consecutive idle frames (zero input + zero animations). Saturates at u32::MAX.
static CONSECUTIVE_IDLE_FRAMES: AtomicU32 = AtomicU32::new(0);
/// 30 frames ≈ 0.5 s at 60 fps — long enough to filter transient quiet
/// moments mid-interaction, short enough that workers wake quickly.
const IDLE_FRAME_THRESHOLD: u32 = 30;
/// Storm detection: warn if a single frame logs more than this many requests.
const REPAINT_STORM_THRESHOLD: u32 = 10;
/// Rate-limit storm warnings to once per minute (in monotonic seconds since process start).
static LAST_STORM_WARN_SECS: Mutex<Option<u64>> = Mutex::new(None);
static PROCESS_START: Mutex<Option<Instant>> = Mutex::new(None);

/// Record a request_repaint / request_redraw call. Pass `concat!(file!(), ":",
/// line!())` (or any `&'static str`) so the storm warning can name the
/// offender.
pub fn note_repaint(caller: &'static str) {
    REPAINT_COUNT.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut g) = LAST_CALLER.lock() {
        *g = Some(caller);
    }
}

/// Tell the profiler how many input events egui saw this frame. Call once at
/// frame begin (after `frame_begin`) with `ctx.input(|i| i.events.len())`.
pub fn note_input_events(n: u32) {
    INPUT_EVENT_COUNT.store(n, Ordering::Relaxed);
}

/// Increment the in-flight animation counter. Pair with `animation_finished`.
/// Motion helpers (motion::ease_*) should bump this when they kick off a
/// transition and decrement when the transition lands.
pub fn animation_started() {
    ANIMATIONS_IN_FLIGHT.fetch_add(1, Ordering::Relaxed);
}
pub fn animation_finished() {
    ANIMATIONS_IN_FLIGHT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some(v.saturating_sub(1)))
        .ok();
}

/// True when the last `IDLE_FRAME_THRESHOLD` frames had no input and no
/// animations. Background workers can use this to batch/defer:
///
///   • drawing_db save worker — when idle, defer flush by 5 s to coalesce
///     multiple edits into one disk write.
///   • apex_data::live_state — when idle, batch state updates instead of
///     pushing each tick to the UI.
///
/// These integrations live OUTSIDE this file's scope; this function is the
/// contract they should call.
pub fn is_idle() -> bool {
    CONSECUTIVE_IDLE_FRAMES.load(Ordering::Relaxed) >= IDLE_FRAME_THRESHOLD
}

/// Most recent frame's repaint stats. Read by perf HUD (future work).
pub fn last_frame_repaints() -> RepaintStats {
    if let Ok(h) = HISTORY.lock() {
        h.last_repaints
    } else {
        RepaintStats::default()
    }
}

/// Thread-local frame state. Lives on the render thread.
struct ThreadState {
    frame_start: Instant,
    depth: u8,
    /// Capacity-bounded zone buffer. We use a fixed `Vec` with `with_capacity`
    /// so push is amortized O(1) and never reallocates while under
    /// `MAX_ZONES_PER_FRAME`.
    current_zones: Vec<ZoneSample>,
}

impl ThreadState {
    fn new() -> Self {
        Self {
            frame_start: Instant::now(),
            depth: 0,
            current_zones: Vec::with_capacity(MAX_ZONES_PER_FRAME),
        }
    }
}

thread_local! {
    static FRAME_STATE: RefCell<ThreadState> = RefCell::new(ThreadState::new());
}

/// Cross-thread snapshot store. The render thread is both writer and the
/// only reader (perf HUD), so contention is nil — but a `Mutex` lets external
/// telemetry pull the data if we ever want it.
struct History {
    frames: Vec<FrameSnapshot>,
    last_zones: Vec<ZoneSample>,
    last_repaints: RepaintStats,
    next_frame_index: u64,
}

static HISTORY: Mutex<History> = Mutex::new(History {
    frames: Vec::new(),
    last_zones: Vec::new(),
    last_repaints: RepaintStats { count: 0, last_caller: None },
    next_frame_index: 0,
});

pub struct ZoneGuard {
    name: &'static str,
    start: Instant,
    depth: u8,
}

impl Drop for ZoneGuard {
    fn drop(&mut self) {
        FRAME_STATE.with(|s| {
            let mut s = s.borrow_mut();
            let duration_us = self.start.elapsed().as_micros() as u64;
            if s.current_zones.len() < MAX_ZONES_PER_FRAME {
                let start_us = self
                    .start
                    .saturating_duration_since(s.frame_start)
                    .as_micros() as u64;
                s.current_zones.push(ZoneSample {
                    name: self.name,
                    start_us,
                    duration_us,
                    depth: self.depth,
                });
            }
            if s.depth > 0 {
                s.depth -= 1;
            }
        });
    }
}

/// Open a new profile zone. The returned guard records the elapsed time
/// when dropped. Cheap: one Instant::now + one bookkeeping borrow.
pub fn profile_zone(name: &'static str) -> ZoneGuard {
    let depth = FRAME_STATE.with(|s| {
        let mut s = s.borrow_mut();
        let d = s.depth;
        s.depth = s.depth.saturating_add(1);
        d
    });
    ZoneGuard {
        name,
        start: Instant::now(),
        depth,
    }
}

/// Marks the start of a new frame (call once at top of redraw).
pub fn frame_begin() {
    FRAME_STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.frame_start = Instant::now();
        s.depth = 0;
        s.current_zones.clear();
    });
    // Reset per-frame counters. note_repaint() / note_input_events() will
    // populate these between now and frame_end().
    REPAINT_COUNT.store(0, Ordering::Relaxed);
    INPUT_EVENT_COUNT.store(0, Ordering::Relaxed);
    if let Ok(mut g) = LAST_CALLER.lock() {
        *g = None;
    }
    // Lazily seed the process-start clock so storm rate-limiting has a base.
    if let Ok(mut ps) = PROCESS_START.lock() {
        if ps.is_none() {
            *ps = Some(Instant::now());
        }
    }
}

/// Marks the end of a frame, rolls zones into history. Returns snapshot.
pub fn frame_end() -> FrameSnapshot {
    let (snapshot, zones) = FRAME_STATE.with(|s| {
        let mut s = s.borrow_mut();
        let total_us = s.frame_start.elapsed().as_micros() as u64;
        let zone_count = s.current_zones.len() as u16;
        // Move zones out (keeps capacity for next frame).
        let zones: Vec<ZoneSample> = s.current_zones.drain(..).collect();
        s.depth = 0;
        (
            FrameSnapshot {
                frame_index: 0, // filled in below under the lock
                total_us,
                zone_count,
            },
            zones,
        )
    });

    // Snapshot per-frame repaint + input counters before they get reset on
    // the next frame_begin.
    let repaint_count = REPAINT_COUNT.load(Ordering::Relaxed);
    let last_caller = LAST_CALLER.lock().ok().and_then(|g| *g);
    let input_events = INPUT_EVENT_COUNT.load(Ordering::Relaxed);
    let anims = ANIMATIONS_IN_FLIGHT.load(Ordering::Relaxed);

    // Idle-frame tracker: a frame is "idle" if no input events fired and no
    // animation is in flight. Streaks of IDLE_FRAME_THRESHOLD let is_idle()
    // return true.
    if input_events == 0 && anims == 0 {
        CONSECUTIVE_IDLE_FRAMES
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some(v.saturating_add(1)))
            .ok();
    } else {
        CONSECUTIVE_IDLE_FRAMES.store(0, Ordering::Relaxed);
    }

    // Roll into shared history. Lock contention is nil (single writer thread).
    let snap_out = if let Ok(mut h) = HISTORY.lock() {
        let idx = h.next_frame_index;
        h.next_frame_index = h.next_frame_index.wrapping_add(1);
        let snap = FrameSnapshot {
            frame_index: idx,
            ..snapshot
        };
        h.frames.push(snap);
        if h.frames.len() > HISTORY_DEPTH {
            let drop_n = h.frames.len() - HISTORY_DEPTH;
            h.frames.drain(..drop_n);
        }
        h.last_zones = zones;
        h.last_repaints = RepaintStats { count: repaint_count, last_caller };
        snap
    } else {
        snapshot
    };

    // Storm detection: warn (rate-limited) if a single frame logged more than
    // REPAINT_STORM_THRESHOLD requests. This catches accidental
    // request_repaint loops without spamming logs.
    if repaint_count > REPAINT_STORM_THRESHOLD {
        let now_secs = PROCESS_START
            .lock()
            .ok()
            .and_then(|g| *g)
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        let mut emit = false;
        if let Ok(mut last) = LAST_STORM_WARN_SECS.lock() {
            if last.map_or(true, |prev| now_secs.saturating_sub(prev) >= 60) {
                *last = Some(now_secs);
                emit = true;
            }
        }
        if emit {
            eprintln!(
                "[frame-profiler] WARN: {} repaint requests in frame {} (last from {})",
                repaint_count,
                snap_out.frame_index,
                last_caller.unwrap_or("<unknown>")
            );
        }
    }

    snap_out
}

/// Read recent frames for HUD display (newest last).
pub fn recent_frames(n: usize) -> Vec<FrameSnapshot> {
    if let Ok(h) = HISTORY.lock() {
        let len = h.frames.len();
        let start = len.saturating_sub(n);
        h.frames[start..].to_vec()
    } else {
        Vec::new()
    }
}

/// Read zones from the most recent frame.
pub fn last_frame_zones() -> Vec<ZoneSample> {
    if let Ok(h) = HISTORY.lock() {
        h.last_zones.clone()
    } else {
        Vec::new()
    }
}
