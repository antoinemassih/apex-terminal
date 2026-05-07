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
use std::time::Instant;

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
    next_frame_index: u64,
}

static HISTORY: Mutex<History> = Mutex::new(History {
    frames: Vec::new(),
    last_zones: Vec::new(),
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

    // Roll into shared history. Lock contention is nil (single writer thread).
    if let Ok(mut h) = HISTORY.lock() {
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
        snap
    } else {
        snapshot
    }
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
