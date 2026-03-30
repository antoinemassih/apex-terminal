//! Low-level system & render profiling — GPU (NVML), CPU, memory, frame phases, jank detection.
//!
//! Spawns a background thread that samples hardware metrics every 2 seconds
//! and exposes them via a Prometheus-compatible HTTP endpoint on port 9091.
//!
//! The render loop calls frame_begin() / frame_end_detailed() each frame to
//! record per-phase timings (acquire, layout, tessellate, upload, render, present),
//! vertex/index counts, and paint job stats.
//!
//! A custom global allocator wrapper counts heap allocations and bytes to detect
//! allocation-heavy frames and memory bloat.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use std::cell::RefCell;

// ─── Subsystem profiler (per-frame named spans inside draw_chart) ────────────

/// Per-frame subsystem timing collected in the render thread.
/// Zero-cost when not read — just pushes to a thread-local vec.
#[derive(Clone, Default, Debug)]
pub struct SubsystemProfile {
    pub spans: Vec<(String, u64)>, // (name, microseconds)
}

thread_local! {
    static CURRENT_PROFILE: RefCell<SubsystemProfile> = RefCell::new(SubsystemProfile::default());
    static SPAN_START: RefCell<Option<(String, Instant)>> = RefCell::new(None);
}

/// Begin a named subsystem span. Must call `span_end()` before next `span_begin()`.
pub fn span_begin(name: &str) {
    SPAN_START.with(|s| {
        // Auto-end previous span if still open
        let prev = s.borrow_mut().take();
        if let Some((prev_name, prev_start)) = prev {
            CURRENT_PROFILE.with(|p| {
                p.borrow_mut().spans.push((prev_name, prev_start.elapsed().as_micros() as u64));
            });
        }
        *s.borrow_mut() = Some((name.to_string(), Instant::now()));
    });
}

/// End current subsystem span.
pub fn span_end() {
    SPAN_START.with(|s| {
        if let Some((name, start)) = s.borrow_mut().take() {
            CURRENT_PROFILE.with(|p| {
                p.borrow_mut().spans.push((name, start.elapsed().as_micros() as u64));
            });
        }
    });
}

/// Collect and reset the current frame's subsystem profile.
fn take_profile() -> SubsystemProfile {
    CURRENT_PROFILE.with(|p| {
        let mut profile = p.borrow_mut();
        let result = profile.clone();
        profile.spans.clear();
        result
    })
}

// ─── Allocation tracker (global allocator wrapper) ───────────────────────────

/// Counts every heap allocation/deallocation and bytes moved.
/// Wraps the system allocator with zero overhead on the hot path (just atomics).
pub struct CountingAlloc;

static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static DEALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
// Per-frame counters — reset at frame_begin(), read at frame_end()
static FRAME_ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static FRAME_ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl std::alloc::GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let ptr = unsafe { std::alloc::System.alloc(layout) };
        if !ptr.is_null() {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            FRAME_ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            FRAME_ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        DEALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        DEALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { std::alloc::System.dealloc(ptr, layout) };
    }
}

// ─── Metric snapshots ────────────────────────────────────────────────────────

#[derive(Clone, Default, Debug)]
pub struct GpuSnapshot {
    pub name: String,
    pub index: u32,
    pub utilization_gpu: u32,
    pub utilization_memory: u32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: u32,
    pub power_draw: u32,    // milliwatts
    pub clock_graphics: u32, // MHz
    pub clock_memory: u32,
    pub clock_sm: u32,
    pub fan_speed: Option<u32>,
}

#[derive(Clone, Default, Debug)]
pub struct ProcessSnapshot {
    pub rss: u64,
    pub virt: u64,
    pub cpu_percent: f32,
    pub handle_count: u32,
}

#[derive(Clone, Default, Debug)]
pub struct SystemSnapshot {
    pub total_memory: u64,
    pub available_memory: u64,
    pub used_memory: u64,
    pub cpu_percent: f32,
}

#[derive(Clone, Default, Debug)]
pub struct FrameStats {
    pub last_frame_us: u64,
    pub avg_frame_us: u64,
    pub max_frame_us: u64,
    pub min_frame_us: u64,
    pub p99_frame_us: u64,
    pub fps: f32,
    pub total_frames: u64,
    pub dropped_frames: u64,
}

/// Per-phase breakdown of a single frame.
#[derive(Clone, Default, Debug)]
pub struct FramePhases {
    pub acquire_us: u64,     // Surface texture acquire
    pub layout_us: u64,      // egui layout + draw_chart logic
    pub tessellate_us: u64,  // Shape tessellation
    pub upload_us: u64,      // Texture + buffer GPU upload
    pub render_us: u64,      // Render pass (GPU draw calls)
    pub present_us: u64,     // Swap chain present
    pub paint_jobs: u32,     // Number of egui paint jobs
    pub vertices: u32,       // Total vertex count
    pub indices: u32,        // Total index count
    pub texture_uploads: u32,
    pub texture_frees: u32,
}

/// Rolling averages of phase timings.
#[derive(Clone, Default, Debug)]
pub struct PhaseStats {
    pub avg_acquire_us: u64,
    pub avg_layout_us: u64,
    pub avg_tessellate_us: u64,
    pub avg_upload_us: u64,
    pub avg_render_us: u64,
    pub avg_present_us: u64,
    pub max_layout_us: u64,  // Worst layout time in window (the likely jank source)
    pub max_render_us: u64,
    pub avg_paint_jobs: u32,
    pub avg_vertices: u32,
    pub avg_indices: u32,
    pub total_texture_uploads: u64,
    pub total_texture_frees: u64,
}

/// Allocation stats snapshot.
#[derive(Clone, Default, Debug)]
pub struct AllocStats {
    pub total_allocs: u64,
    pub total_deallocs: u64,
    pub total_alloc_bytes: u64,
    pub total_dealloc_bytes: u64,
    /// Net bytes currently held (alloc - dealloc)
    pub net_bytes: i64,
    /// Allocations in the last frame
    pub frame_allocs: u64,
    /// Bytes allocated in the last frame
    pub frame_alloc_bytes: u64,
    /// Average allocs per frame (rolling)
    pub avg_frame_allocs: u64,
    /// Average bytes per frame (rolling)
    pub avg_frame_alloc_bytes: u64,
}

/// Rolling averages of subsystem timings inside draw_chart.
#[derive(Clone, Default, Debug)]
pub struct SubsystemStats {
    /// name → (avg_us, max_us, last_us)
    pub spans: Vec<(String, u64, u64, u64)>,
}

/// Jank event — a frame that exceeded a threshold.
#[derive(Clone, Debug)]
pub struct JankEvent {
    pub frame_number: u64,
    pub total_us: u64,
    pub phases: FramePhases,
    pub subsystems: Vec<(String, u64)>, // subsystem breakdown at time of jank
    pub allocs_in_frame: u64,
    pub alloc_bytes_in_frame: u64,
    pub timestamp_secs: u64, // seconds since monitoring start
}

#[derive(Clone, Default, Debug)]
pub struct LeakDetector {
    samples: Vec<u64>,
    pub leak_suspected: bool,
    pub consecutive_increases: u32,
    pub baseline_rss: u64,
    pub growth_from_baseline: i64,
}

#[derive(Clone, Default, Debug)]
pub struct Snapshot {
    pub gpus: Vec<GpuSnapshot>,
    pub process: ProcessSnapshot,
    pub system: SystemSnapshot,
    pub frames: FrameStats,
    pub phases: PhaseStats,
    pub allocs: AllocStats,
    pub subsystems: SubsystemStats,
    pub leak: LeakDetector,
    pub jank_events: Vec<JankEvent>, // Last 20 jank events
    pub uptime_secs: u64,
}

// ─── Global state ────────────────────────────────────────────────────────────

static METRICS: OnceLock<Arc<Mutex<Snapshot>>> = OnceLock::new();
static FRAME_TRACKER: OnceLock<Arc<Mutex<FrameTracker>>> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();

fn metrics() -> &'static Arc<Mutex<Snapshot>> {
    METRICS.get_or_init(|| Arc::new(Mutex::new(Snapshot::default())))
}

fn frame_tracker() -> &'static Arc<Mutex<FrameTracker>> {
    FRAME_TRACKER.get_or_init(|| Arc::new(Mutex::new(FrameTracker::new())))
}

fn start_time() -> &'static Instant {
    START_TIME.get_or_init(Instant::now)
}

/// Call at the start of each render frame.
pub fn frame_begin() {
    // Reset per-frame allocation counters
    FRAME_ALLOC_COUNT.store(0, Ordering::Relaxed);
    FRAME_ALLOC_BYTES.store(0, Ordering::Relaxed);
    if let Ok(mut ft) = frame_tracker().lock() {
        ft.begin();
    }
}

/// Call at the end of each render frame with detailed phase timings.
pub fn frame_end_detailed(phases: FramePhases) {
    let frame_allocs = FRAME_ALLOC_COUNT.load(Ordering::Relaxed);
    let frame_alloc_bytes = FRAME_ALLOC_BYTES.load(Ordering::Relaxed);
    if let Ok(mut ft) = frame_tracker().lock() {
        ft.end_detailed(phases, frame_allocs, frame_alloc_bytes);
    }
}

/// Get current snapshot for external use.
pub fn current_snapshot() -> Snapshot {
    metrics().lock().map(|s| s.clone()).unwrap_or_default()
}

// ─── Frame tracker with phase profiling ──────────────────────────────────────

const RING_SIZE: usize = 300; // ~5s at 60fps
const JANK_THRESHOLD_US: u64 = 20_000; // 20ms = below 50fps = jank

struct FrameTracker {
    frame_start: Option<Instant>,
    // Total frame time ring
    ring: Vec<u64>,
    ring_pos: usize,
    total_frames: u64,
    dropped_frames: u64,
    // Phase rings
    acquire_ring: Vec<u64>,
    layout_ring: Vec<u64>,
    tessellate_ring: Vec<u64>,
    upload_ring: Vec<u64>,
    render_ring: Vec<u64>,
    present_ring: Vec<u64>,
    // Render stats rings
    paint_jobs_ring: Vec<u32>,
    vertices_ring: Vec<u32>,
    indices_ring: Vec<u32>,
    total_tex_uploads: u64,
    total_tex_frees: u64,
    // Per-frame alloc rings
    alloc_count_ring: Vec<u64>,
    alloc_bytes_ring: Vec<u64>,
    // Jank log
    jank_events: Vec<JankEvent>,
    // Subsystem profiling — last N profiles for rolling stats
    subsystem_ring: Vec<SubsystemProfile>,
    subsystem_ring_pos: usize,
}

impl FrameTracker {
    fn new() -> Self {
        Self {
            frame_start: None,
            ring: vec![0; RING_SIZE],
            ring_pos: 0,
            total_frames: 0,
            dropped_frames: 0,
            acquire_ring: vec![0; RING_SIZE],
            layout_ring: vec![0; RING_SIZE],
            tessellate_ring: vec![0; RING_SIZE],
            upload_ring: vec![0; RING_SIZE],
            render_ring: vec![0; RING_SIZE],
            present_ring: vec![0; RING_SIZE],
            paint_jobs_ring: vec![0; RING_SIZE],
            vertices_ring: vec![0; RING_SIZE],
            indices_ring: vec![0; RING_SIZE],
            total_tex_uploads: 0,
            total_tex_frees: 0,
            alloc_count_ring: vec![0; RING_SIZE],
            alloc_bytes_ring: vec![0; RING_SIZE],
            jank_events: Vec::new(),
            subsystem_ring: (0..RING_SIZE).map(|_| SubsystemProfile::default()).collect(),
            subsystem_ring_pos: 0,
        }
    }

    fn begin(&mut self) {
        self.frame_start = Some(Instant::now());
    }

    fn end_detailed(&mut self, phases: FramePhases, frame_allocs: u64, frame_alloc_bytes: u64) {
        let total_us = if let Some(start) = self.frame_start.take() {
            start.elapsed().as_micros() as u64
        } else {
            phases.acquire_us + phases.layout_us + phases.tessellate_us
                + phases.upload_us + phases.render_us + phases.present_us
        };

        // Capture subsystem profile from this frame's thread-local spans
        let profile = take_profile();

        let pos = self.ring_pos;
        self.ring[pos] = total_us;
        self.acquire_ring[pos] = phases.acquire_us;
        self.layout_ring[pos] = phases.layout_us;
        self.tessellate_ring[pos] = phases.tessellate_us;
        self.upload_ring[pos] = phases.upload_us;
        self.render_ring[pos] = phases.render_us;
        self.present_ring[pos] = phases.present_us;
        self.paint_jobs_ring[pos] = phases.paint_jobs;
        self.vertices_ring[pos] = phases.vertices;
        self.indices_ring[pos] = phases.indices;
        self.total_tex_uploads += phases.texture_uploads as u64;
        self.total_tex_frees += phases.texture_frees as u64;
        self.alloc_count_ring[pos] = frame_allocs;
        self.alloc_bytes_ring[pos] = frame_alloc_bytes;
        self.subsystem_ring[self.subsystem_ring_pos] = profile.clone();
        self.subsystem_ring_pos = (self.subsystem_ring_pos + 1) % RING_SIZE;

        self.ring_pos = (pos + 1) % RING_SIZE;
        self.total_frames += 1;
        if total_us > 33_333 { self.dropped_frames += 1; }

        // Jank detection — include subsystem breakdown for root cause
        if total_us > JANK_THRESHOLD_US {
            let event = JankEvent {
                frame_number: self.total_frames,
                total_us,
                phases,
                subsystems: profile.spans.clone(),
                allocs_in_frame: frame_allocs,
                alloc_bytes_in_frame: frame_alloc_bytes,
                timestamp_secs: start_time().elapsed().as_secs(),
            };
            self.jank_events.push(event);
            if self.jank_events.len() > 50 {
                self.jank_events.remove(0);
            }
        }
    }

    fn frame_stats(&self) -> FrameStats {
        let filled = self.total_frames.min(RING_SIZE as u64) as usize;
        if filled == 0 {
            return FrameStats { total_frames: self.total_frames, dropped_frames: self.dropped_frames, ..Default::default() };
        }
        let mut sorted: Vec<u64> = if self.total_frames >= RING_SIZE as u64 {
            self.ring.clone()
        } else {
            self.ring[..filled].to_vec()
        };
        sorted.sort_unstable();
        let sum: u64 = sorted.iter().sum();
        let avg = sum / filled as u64;
        let p99_idx = ((filled as f64) * 0.99).floor() as usize;
        FrameStats {
            last_frame_us: self.ring[(self.ring_pos + RING_SIZE - 1) % RING_SIZE],
            avg_frame_us: avg,
            max_frame_us: *sorted.last().unwrap_or(&0),
            min_frame_us: sorted[0],
            p99_frame_us: sorted[p99_idx.min(filled - 1)],
            fps: if avg > 0 { 1_000_000.0 / avg as f32 } else { 0.0 },
            total_frames: self.total_frames,
            dropped_frames: self.dropped_frames,
        }
    }

    fn phase_stats(&self) -> PhaseStats {
        let filled = self.total_frames.min(RING_SIZE as u64) as usize;
        if filled == 0 { return PhaseStats::default(); }

        let avg = |ring: &[u64]| -> u64 {
            let slice = if filled >= RING_SIZE { ring } else { &ring[..filled] };
            slice.iter().sum::<u64>() / filled as u64
        };
        let max = |ring: &[u64]| -> u64 {
            let slice = if filled >= RING_SIZE { ring } else { &ring[..filled] };
            *slice.iter().max().unwrap_or(&0)
        };
        let avg_u32 = |ring: &[u32]| -> u32 {
            let slice = if filled >= RING_SIZE { ring } else { &ring[..filled] };
            (slice.iter().map(|v| *v as u64).sum::<u64>() / filled as u64) as u32
        };

        PhaseStats {
            avg_acquire_us: avg(&self.acquire_ring),
            avg_layout_us: avg(&self.layout_ring),
            avg_tessellate_us: avg(&self.tessellate_ring),
            avg_upload_us: avg(&self.upload_ring),
            avg_render_us: avg(&self.render_ring),
            avg_present_us: avg(&self.present_ring),
            max_layout_us: max(&self.layout_ring),
            max_render_us: max(&self.render_ring),
            avg_paint_jobs: avg_u32(&self.paint_jobs_ring),
            avg_vertices: avg_u32(&self.vertices_ring),
            avg_indices: avg_u32(&self.indices_ring),
            total_texture_uploads: self.total_tex_uploads,
            total_texture_frees: self.total_tex_frees,
        }
    }

    fn alloc_stats(&self) -> AllocStats {
        let filled = self.total_frames.min(RING_SIZE as u64) as usize;
        let (avg_allocs, avg_bytes) = if filled > 0 {
            let slice_a = if filled >= RING_SIZE { &self.alloc_count_ring[..] } else { &self.alloc_count_ring[..filled] };
            let slice_b = if filled >= RING_SIZE { &self.alloc_bytes_ring[..] } else { &self.alloc_bytes_ring[..filled] };
            (
                slice_a.iter().sum::<u64>() / filled as u64,
                slice_b.iter().sum::<u64>() / filled as u64,
            )
        } else { (0, 0) };

        let total_a = ALLOC_BYTES.load(Ordering::Relaxed);
        let total_d = DEALLOC_BYTES.load(Ordering::Relaxed);

        AllocStats {
            total_allocs: ALLOC_COUNT.load(Ordering::Relaxed),
            total_deallocs: DEALLOC_COUNT.load(Ordering::Relaxed),
            total_alloc_bytes: total_a,
            total_dealloc_bytes: total_d,
            net_bytes: total_a as i64 - total_d as i64,
            frame_allocs: FRAME_ALLOC_COUNT.load(Ordering::Relaxed),
            frame_alloc_bytes: FRAME_ALLOC_BYTES.load(Ordering::Relaxed),
            avg_frame_allocs: avg_allocs,
            avg_frame_alloc_bytes: avg_bytes,
        }
    }

    fn subsystem_stats(&self) -> SubsystemStats {
        let filled = self.total_frames.min(RING_SIZE as u64) as usize;
        if filled == 0 { return SubsystemStats::default(); }

        // Collect all span names and accumulate
        let mut totals: std::collections::HashMap<String, (u64, u64, u64, u64)> = std::collections::HashMap::new(); // sum, max, last, count
        let ring_len = self.subsystem_ring.len();
        for i in 0..filled.min(ring_len) {
            let idx = if filled >= RING_SIZE { i } else { i };
            for (name, us) in &self.subsystem_ring[idx].spans {
                let e = totals.entry(name.clone()).or_insert((0, 0, 0, 0));
                e.0 += us;
                if *us > e.1 { e.1 = *us; }
                e.2 = *us; // last
                e.3 += 1;
            }
        }

        let mut spans: Vec<(String, u64, u64, u64)> = totals.into_iter()
            .map(|(name, (sum, max, last, count))| {
                let avg = if count > 0 { sum / count } else { 0 };
                (name, avg, max, last)
            })
            .collect();
        spans.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by avg descending (slowest first)
        SubsystemStats { spans }
    }

    fn jank_events(&self) -> Vec<JankEvent> {
        let start = if self.jank_events.len() > 20 { self.jank_events.len() - 20 } else { 0 };
        self.jank_events[start..].to_vec()
    }
}

// ─── Leak detector ───────────────────────────────────────────────────────────

impl LeakDetector {
    fn update(&mut self, rss: u64) {
        if self.baseline_rss == 0 { self.baseline_rss = rss; }
        self.growth_from_baseline = rss as i64 - self.baseline_rss as i64;
        if let Some(&last) = self.samples.last() {
            if rss > last { self.consecutive_increases += 1; } else { self.consecutive_increases = 0; }
        }
        self.samples.push(rss);
        if self.samples.len() > 300 { self.samples.remove(0); }
        self.leak_suspected = self.consecutive_increases >= 60;
    }
}

// ─── NVML GPU polling ────────────────────────────────────────────────────────

#[cfg(windows)]
mod nvml {
    use std::ffi::CStr;
    use std::os::raw::c_char;

    type NvmlReturn = u32;
    const NVML_SUCCESS: NvmlReturn = 0;

    #[repr(C)]
    pub struct NvmlUtilization { pub gpu: u32, pub memory: u32 }

    #[repr(C)]
    pub struct NvmlMemory { pub total: u64, pub free: u64, pub used: u64 }

    type NvmlDevice = *mut std::ffi::c_void;

    #[allow(dead_code)]
    pub struct Nvml {
        _lib: libloading::Library,
        device_count: unsafe extern "C" fn(*mut u32) -> NvmlReturn,
        device_get_handle: unsafe extern "C" fn(u32, *mut NvmlDevice) -> NvmlReturn,
        device_get_name: unsafe extern "C" fn(NvmlDevice, *mut c_char, u32) -> NvmlReturn,
        device_get_utilization: unsafe extern "C" fn(NvmlDevice, *mut NvmlUtilization) -> NvmlReturn,
        device_get_memory: unsafe extern "C" fn(NvmlDevice, *mut NvmlMemory) -> NvmlReturn,
        device_get_temperature: unsafe extern "C" fn(NvmlDevice, u32, *mut u32) -> NvmlReturn,
        device_get_power: unsafe extern "C" fn(NvmlDevice, *mut u32) -> NvmlReturn,
        device_get_clock: unsafe extern "C" fn(NvmlDevice, u32, *mut u32) -> NvmlReturn,
        device_get_fan: unsafe extern "C" fn(NvmlDevice, *mut u32) -> NvmlReturn,
        handles: Vec<NvmlDevice>,
    }

    unsafe impl Send for Nvml {}
    unsafe impl Sync for Nvml {}

    impl Nvml {
        pub fn init() -> Option<Self> {
            unsafe {
                let lib = libloading::Library::new("nvml.dll").ok()?;
                let init: libloading::Symbol<unsafe extern "C" fn() -> NvmlReturn> = lib.get(b"nvmlInit_v2\0").ok()?;
                if init() != NVML_SUCCESS { return None; }

                let device_count: unsafe extern "C" fn(*mut u32) -> NvmlReturn = *lib.get(b"nvmlDeviceGetCount_v2\0").ok()?;
                let device_get_handle: unsafe extern "C" fn(u32, *mut NvmlDevice) -> NvmlReturn = *lib.get(b"nvmlDeviceGetHandleByIndex_v2\0").ok()?;
                let device_get_name: unsafe extern "C" fn(NvmlDevice, *mut c_char, u32) -> NvmlReturn = *lib.get(b"nvmlDeviceGetName\0").ok()?;
                let device_get_utilization: unsafe extern "C" fn(NvmlDevice, *mut NvmlUtilization) -> NvmlReturn = *lib.get(b"nvmlDeviceGetUtilizationRates\0").ok()?;
                let device_get_memory: unsafe extern "C" fn(NvmlDevice, *mut NvmlMemory) -> NvmlReturn = *lib.get(b"nvmlDeviceGetMemoryInfo\0").ok()?;
                let device_get_temperature: unsafe extern "C" fn(NvmlDevice, u32, *mut u32) -> NvmlReturn = *lib.get(b"nvmlDeviceGetTemperature\0").ok()?;
                let device_get_power: unsafe extern "C" fn(NvmlDevice, *mut u32) -> NvmlReturn = *lib.get(b"nvmlDeviceGetPowerUsage\0").ok()?;
                let device_get_clock: unsafe extern "C" fn(NvmlDevice, u32, *mut u32) -> NvmlReturn = *lib.get(b"nvmlDeviceGetClockInfo\0").ok()?;
                let device_get_fan: unsafe extern "C" fn(NvmlDevice, *mut u32) -> NvmlReturn = *lib.get(b"nvmlDeviceGetFanSpeed\0").ok()?;

                let mut count = 0u32;
                device_count(&mut count);
                let mut handles = Vec::new();
                for i in 0..count {
                    let mut dev: NvmlDevice = std::ptr::null_mut();
                    if device_get_handle(i, &mut dev) == NVML_SUCCESS { handles.push(dev); }
                }

                Some(Self {
                    _lib: lib, device_count, device_get_handle, device_get_name,
                    device_get_utilization, device_get_memory, device_get_temperature,
                    device_get_power, device_get_clock, device_get_fan, handles,
                })
            }
        }

        pub fn poll(&self) -> Vec<super::GpuSnapshot> {
            self.handles.iter().enumerate().map(|(i, &dev)| {
                let mut snap = super::GpuSnapshot { index: i as u32, ..Default::default() };
                unsafe {
                    let mut name_buf = [0i8; 256];
                    if (self.device_get_name)(dev, name_buf.as_mut_ptr(), 256) == NVML_SUCCESS {
                        snap.name = CStr::from_ptr(name_buf.as_ptr()).to_string_lossy().to_string();
                    }
                    let mut util = NvmlUtilization { gpu: 0, memory: 0 };
                    if (self.device_get_utilization)(dev, &mut util) == NVML_SUCCESS {
                        snap.utilization_gpu = util.gpu; snap.utilization_memory = util.memory;
                    }
                    let mut mem = NvmlMemory { total: 0, free: 0, used: 0 };
                    if (self.device_get_memory)(dev, &mut mem) == NVML_SUCCESS {
                        snap.memory_used = mem.used; snap.memory_total = mem.total;
                    }
                    let mut temp = 0u32;
                    if (self.device_get_temperature)(dev, 0, &mut temp) == NVML_SUCCESS { snap.temperature = temp; }
                    let mut power = 0u32;
                    if (self.device_get_power)(dev, &mut power) == NVML_SUCCESS { snap.power_draw = power; }
                    let mut clk = 0u32;
                    if (self.device_get_clock)(dev, 0, &mut clk) == NVML_SUCCESS { snap.clock_graphics = clk; }
                    if (self.device_get_clock)(dev, 1, &mut clk) == NVML_SUCCESS { snap.clock_sm = clk; }
                    if (self.device_get_clock)(dev, 2, &mut clk) == NVML_SUCCESS { snap.clock_memory = clk; }
                    let mut fan = 0u32;
                    if (self.device_get_fan)(dev, &mut fan) == NVML_SUCCESS { snap.fan_speed = Some(fan); }
                }
                snap
            }).collect()
        }
    }

    impl Drop for Nvml {
        fn drop(&mut self) {
            unsafe {
                if let Ok(shutdown) = self._lib.get::<unsafe extern "C" fn() -> NvmlReturn>(b"nvmlShutdown\0") {
                    shutdown();
                }
            }
        }
    }
}

// ─── Process metrics (Windows) ───────────────────────────────────────────────

#[cfg(windows)]
fn poll_process() -> ProcessSnapshot {
    use std::mem;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct PROCESS_MEMORY_COUNTERS_EX {
        cb: u32, PageFaultCount: u32, PeakWorkingSetSize: usize, WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize, QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize, QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize, PeakPagefileUsage: usize, PrivateUsage: usize,
    }

    #[link(name = "psapi")]
    extern "system" {
        fn GetProcessMemoryInfo(process: *mut std::ffi::c_void, ppsmemCounters: *mut PROCESS_MEMORY_COUNTERS_EX, cb: u32) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn GetProcessHandleCount(process: *mut std::ffi::c_void, count: *mut u32) -> i32;
    }

    unsafe {
        let handle = GetCurrentProcess();
        let mut pmc: PROCESS_MEMORY_COUNTERS_EX = mem::zeroed();
        pmc.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let mut snap = ProcessSnapshot::default();
        if GetProcessMemoryInfo(handle, &mut pmc, pmc.cb) != 0 {
            snap.rss = pmc.WorkingSetSize as u64;
            snap.virt = pmc.PrivateUsage as u64;
        }
        let mut hcount = 0u32;
        if GetProcessHandleCount(handle, &mut hcount) != 0 { snap.handle_count = hcount; }
        snap
    }
}

#[cfg(windows)]
fn poll_system() -> SystemSnapshot {
    use std::mem;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct MEMORYSTATUSEX {
        dwLength: u32, dwMemoryLoad: u32,
        ullTotalPhys: u64, ullAvailPhys: u64, ullTotalPageFile: u64, ullAvailPageFile: u64,
        ullTotalVirtual: u64, ullAvailVirtual: u64, ullAvailExtendedVirtual: u64,
    }

    #[link(name = "kernel32")]
    extern "system" { fn GlobalMemoryStatusEx(lpBuffer: *mut MEMORYSTATUSEX) -> i32; }

    unsafe {
        let mut status: MEMORYSTATUSEX = mem::zeroed();
        status.dwLength = mem::size_of::<MEMORYSTATUSEX>() as u32;
        let mut snap = SystemSnapshot::default();
        if GlobalMemoryStatusEx(&mut status) != 0 {
            snap.total_memory = status.ullTotalPhys;
            snap.available_memory = status.ullAvailPhys;
            snap.used_memory = status.ullTotalPhys - status.ullAvailPhys;
        }
        snap
    }
}

// ─── CPU usage tracking ─────────────────────────────────────────────────────

#[cfg(windows)]
struct CpuTracker { prev_idle: u64, prev_kernel: u64, prev_user: u64, prev_pk: u64, prev_pu: u64 }

#[cfg(windows)]
impl CpuTracker {
    fn new() -> Self { Self { prev_idle: 0, prev_kernel: 0, prev_user: 0, prev_pk: 0, prev_pu: 0 } }

    fn poll(&mut self) -> (f32, f32) {
        #[repr(C)]
        #[allow(non_snake_case)]
        struct FILETIME { dwLowDateTime: u32, dwHighDateTime: u32 }
        fn ft(ft: &FILETIME) -> u64 { (ft.dwHighDateTime as u64) << 32 | ft.dwLowDateTime as u64 }

        #[link(name = "kernel32")]
        extern "system" {
            fn GetSystemTimes(idle: *mut FILETIME, kernel: *mut FILETIME, user: *mut FILETIME) -> i32;
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetProcessTimes(p: *mut std::ffi::c_void, c: *mut FILETIME, e: *mut FILETIME, k: *mut FILETIME, u: *mut FILETIME) -> i32;
        }

        unsafe {
            let (mut idle, mut kernel, mut user) = (std::mem::zeroed(), std::mem::zeroed(), std::mem::zeroed());
            let mut sys_cpu = 0.0f32;
            let mut proc_cpu = 0.0f32;

            if GetSystemTimes(&mut idle, &mut kernel, &mut user) != 0 {
                let (i, k, u) = (ft(&idle), ft(&kernel), ft(&user));
                let total = (k + u).wrapping_sub(self.prev_kernel + self.prev_user);
                let idle_d = i.wrapping_sub(self.prev_idle);
                if total > 0 { sys_cpu = ((total - idle_d) as f64 / total as f64 * 100.0) as f32; }
                self.prev_idle = i; self.prev_kernel = k; self.prev_user = u;
            }

            let handle = GetCurrentProcess();
            let (mut c, mut e, mut pk, mut pu) = (std::mem::zeroed(), std::mem::zeroed(), std::mem::zeroed(), std::mem::zeroed());
            if GetProcessTimes(handle, &mut c, &mut e, &mut pk, &mut pu) != 0 {
                let (k, u) = (ft(&pk), ft(&pu));
                let proc_d = (k + u).wrapping_sub(self.prev_pk + self.prev_pu);
                let _sys_d = (ft(&kernel) + ft(&user)).wrapping_sub(self.prev_kernel + self.prev_user);
                // Use wallclock-based calc for process CPU to avoid division issues
                let total_sys = (ft(&kernel) + ft(&user)).wrapping_sub(self.prev_kernel + self.prev_user);
                if total_sys > 0 { proc_cpu = (proc_d as f64 / total_sys as f64 * 100.0) as f32; }
                self.prev_pk = k; self.prev_pu = u;
            }
            (sys_cpu, proc_cpu)
        }
    }
}

// ─── Prometheus format ───────────────────────────────────────────────────────

fn format_prometheus(snap: &Snapshot) -> String {
    let mut o = String::with_capacity(8192);

    // GPU
    for g in &snap.gpus {
        let (i, n) = (g.index, &g.name);
        macro_rules! gpu_metric {
            ($name:expr, $help:expr, $val:expr) => {
                o.push_str(&format!("# HELP {} {}\n# TYPE {} gauge\n{}{{gpu=\"{}\",name=\"{}\"}} {}\n",
                    $name, $help, $name, $name, i, n, $val));
            };
        }
        gpu_metric!("apex_gpu_utilization_percent", "GPU compute utilization", g.utilization_gpu);
        gpu_metric!("apex_gpu_mem_utilization_percent", "GPU memory controller utilization", g.utilization_memory);
        gpu_metric!("apex_gpu_memory_used_bytes", "GPU VRAM used", g.memory_used);
        gpu_metric!("apex_gpu_memory_total_bytes", "GPU VRAM total", g.memory_total);
        gpu_metric!("apex_gpu_temperature_celsius", "GPU temperature", g.temperature);
        gpu_metric!("apex_gpu_power_watts", "GPU power draw", format!("{:.1}", g.power_draw as f64 / 1000.0));
        gpu_metric!("apex_gpu_clock_graphics_mhz", "GPU graphics clock", g.clock_graphics);
        gpu_metric!("apex_gpu_clock_memory_mhz", "GPU memory clock", g.clock_memory);
        gpu_metric!("apex_gpu_clock_sm_mhz", "GPU SM clock", g.clock_sm);
        if let Some(fan) = g.fan_speed {
            gpu_metric!("apex_gpu_fan_percent", "GPU fan speed", fan);
        }
    }

    // Process
    macro_rules! metric {
        ($name:expr, $help:expr, $type:expr, $val:expr) => {
            o.push_str(&format!("# HELP {} {}\n# TYPE {} {}\n{} {}\n", $name, $help, $name, $type, $name, $val));
        };
    }
    metric!("apex_process_rss_bytes", "Working set size", "gauge", snap.process.rss);
    metric!("apex_process_virtual_bytes", "Private bytes", "gauge", snap.process.virt);
    metric!("apex_process_cpu_percent", "Process CPU usage", "gauge", format!("{:.2}", snap.process.cpu_percent));
    metric!("apex_process_handles", "Open handles", "gauge", snap.process.handle_count);

    // System
    metric!("apex_system_memory_total_bytes", "Total physical RAM", "gauge", snap.system.total_memory);
    metric!("apex_system_memory_available_bytes", "Available RAM", "gauge", snap.system.available_memory);
    metric!("apex_system_cpu_percent", "System CPU usage", "gauge", format!("{:.2}", snap.system.cpu_percent));

    // Frame timing
    metric!("apex_frame_fps", "Frames per second", "gauge", format!("{:.1}", snap.frames.fps));
    metric!("apex_frame_time_avg_us", "Avg frame time (us)", "gauge", snap.frames.avg_frame_us);
    metric!("apex_frame_time_min_us", "Min frame time (us)", "gauge", snap.frames.min_frame_us);
    metric!("apex_frame_time_max_us", "Max frame time (us)", "gauge", snap.frames.max_frame_us);
    metric!("apex_frame_time_p99_us", "P99 frame time (us)", "gauge", snap.frames.p99_frame_us);
    metric!("apex_frame_time_last_us", "Last frame time (us)", "gauge", snap.frames.last_frame_us);
    metric!("apex_frame_total", "Total frames rendered", "counter", snap.frames.total_frames);
    metric!("apex_frame_dropped_total", "Frames >33ms (jank)", "counter", snap.frames.dropped_frames);

    // Phase breakdown
    metric!("apex_phase_acquire_avg_us", "Avg surface acquire time", "gauge", snap.phases.avg_acquire_us);
    metric!("apex_phase_layout_avg_us", "Avg egui layout time", "gauge", snap.phases.avg_layout_us);
    metric!("apex_phase_layout_max_us", "Worst layout time in window", "gauge", snap.phases.max_layout_us);
    metric!("apex_phase_tessellate_avg_us", "Avg tessellation time", "gauge", snap.phases.avg_tessellate_us);
    metric!("apex_phase_upload_avg_us", "Avg GPU upload time", "gauge", snap.phases.avg_upload_us);
    metric!("apex_phase_render_avg_us", "Avg render pass time", "gauge", snap.phases.avg_render_us);
    metric!("apex_phase_render_max_us", "Worst render pass in window", "gauge", snap.phases.max_render_us);
    metric!("apex_phase_present_avg_us", "Avg present/vsync time", "gauge", snap.phases.avg_present_us);

    // Render stats
    metric!("apex_render_paint_jobs", "Avg paint jobs per frame", "gauge", snap.phases.avg_paint_jobs);
    metric!("apex_render_vertices", "Avg vertices per frame", "gauge", snap.phases.avg_vertices);
    metric!("apex_render_indices", "Avg indices per frame", "gauge", snap.phases.avg_indices);
    metric!("apex_render_texture_uploads_total", "Total texture uploads", "counter", snap.phases.total_texture_uploads);
    metric!("apex_render_texture_frees_total", "Total texture frees", "counter", snap.phases.total_texture_frees);

    // Allocations
    metric!("apex_alloc_total", "Total heap allocations", "counter", snap.allocs.total_allocs);
    metric!("apex_dealloc_total", "Total heap deallocations", "counter", snap.allocs.total_deallocs);
    metric!("apex_alloc_bytes_total", "Total bytes allocated", "counter", snap.allocs.total_alloc_bytes);
    metric!("apex_dealloc_bytes_total", "Total bytes freed", "counter", snap.allocs.total_dealloc_bytes);
    metric!("apex_alloc_net_bytes", "Net heap bytes held (alloc-dealloc)", "gauge", snap.allocs.net_bytes);
    metric!("apex_alloc_frame_count", "Allocations in last frame", "gauge", snap.allocs.frame_allocs);
    metric!("apex_alloc_frame_bytes", "Bytes allocated in last frame", "gauge", snap.allocs.frame_alloc_bytes);
    metric!("apex_alloc_frame_avg_count", "Avg allocations per frame", "gauge", snap.allocs.avg_frame_allocs);
    metric!("apex_alloc_frame_avg_bytes", "Avg bytes per frame", "gauge", snap.allocs.avg_frame_alloc_bytes);

    // Subsystem breakdown (inside draw_chart)
    for (name, avg, max, last) in &snap.subsystems.spans {
        o.push_str(&format!(
            "apex_subsystem_avg_us{{name=\"{}\"}} {}\napex_subsystem_max_us{{name=\"{}\"}} {}\napex_subsystem_last_us{{name=\"{}\"}} {}\n",
            name, avg, name, max, name, last
        ));
    }
    if !snap.subsystems.spans.is_empty() {
        o.push_str("# HELP apex_subsystem_avg_us Avg time per subsystem inside draw_chart\n# TYPE apex_subsystem_avg_us gauge\n");
        o.push_str("# HELP apex_subsystem_max_us Max time per subsystem in window\n# TYPE apex_subsystem_max_us gauge\n");
    }

    // Leak detection
    metric!("apex_leak_suspected", "Memory leak suspected", "gauge", if snap.leak.leak_suspected { 1 } else { 0 });
    metric!("apex_leak_growth_bytes", "RSS growth from baseline", "gauge", snap.leak.growth_from_baseline);
    metric!("apex_leak_consecutive_increases", "Consecutive RSS increases", "gauge", snap.leak.consecutive_increases);

    // Jank events
    metric!("apex_jank_events_total", "Total jank events (>20ms)", "gauge", snap.jank_events.len());

    // Uptime
    metric!("apex_uptime_seconds", "Application uptime", "counter", snap.uptime_secs);

    o
}

/// Format jank events as JSON for the /jank endpoint.
fn format_jank_json(snap: &Snapshot) -> String {
    let events: Vec<String> = snap.jank_events.iter().map(|j| {
        let subsys: Vec<String> = j.subsystems.iter()
            .map(|(name, us)| format!(r#"{{"name":"{}","us":{}}}"#, name, us))
            .collect();
        format!(
            r#"{{"frame":{},"total_us":{},"acquire_us":{},"layout_us":{},"tessellate_us":{},"upload_us":{},"render_us":{},"present_us":{},"paint_jobs":{},"vertices":{},"indices":{},"allocs":{},"alloc_bytes":{},"subsystems":[{}],"at_secs":{}}}"#,
            j.frame_number, j.total_us,
            j.phases.acquire_us, j.phases.layout_us, j.phases.tessellate_us,
            j.phases.upload_us, j.phases.render_us, j.phases.present_us,
            j.phases.paint_jobs, j.phases.vertices, j.phases.indices,
            j.allocs_in_frame, j.alloc_bytes_in_frame,
            subsys.join(","), j.timestamp_secs,
        )
    }).collect();
    format!("[{}]", events.join(","))
}

// ─── HTTP metrics server ─────────────────────────────────────────────────────

fn start_http_server(metrics: Arc<Mutex<Snapshot>>) {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    std::thread::Builder::new().name("metrics-http".into()).spawn(move || {
        let listener = match TcpListener::bind("0.0.0.0:9091") {
            Ok(l) => l,
            Err(e) => { eprintln!("[monitoring] Failed to bind :9091 — {e}"); return; }
        };
        eprintln!("[monitoring] Prometheus metrics at http://0.0.0.0:9091/metrics");
        eprintln!("[monitoring] Jank events at http://0.0.0.0:9091/jank");

        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));

            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);

            let snap = metrics.lock().map(|s| s.clone()).unwrap_or_default();

            let (content_type, body) = if req.contains("GET /jank") {
                ("application/json", format_jank_json(&snap))
            } else {
                ("text/plain; version=0.0.4; charset=utf-8", format_prometheus(&snap))
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    }).expect("Failed to spawn metrics HTTP thread");
}

// ─── Main entry ──────────────────────────────────────────────────────────────

/// Start the monitoring subsystem. Call once at app startup.
pub fn start() {
    let _ = start_time(); // initialize
    let metrics = Arc::clone(metrics());

    start_http_server(Arc::clone(&metrics));

    std::thread::Builder::new().name("monitoring".into()).spawn(move || {
        #[cfg(windows)]
        let nvml_ctx = nvml::Nvml::init();
        #[cfg(windows)]
        if nvml_ctx.is_some() {
            eprintln!("[monitoring] NVML initialized — GPU telemetry active");
        } else {
            eprintln!("[monitoring] NVML not available — GPU metrics disabled");
        }

        #[cfg(windows)]
        let mut cpu_tracker = CpuTracker::new();

        loop {
            std::thread::sleep(Duration::from_secs(2));

            let mut snap = Snapshot::default();
            snap.uptime_secs = start_time().elapsed().as_secs();

            #[cfg(windows)]
            if let Some(ref nvml) = nvml_ctx { snap.gpus = nvml.poll(); }

            #[cfg(windows)]
            { snap.process = poll_process(); }

            #[cfg(windows)]
            {
                snap.system = poll_system();
                let (sys_cpu, proc_cpu) = cpu_tracker.poll();
                snap.system.cpu_percent = sys_cpu;
                snap.process.cpu_percent = proc_cpu;
            }

            if let Ok(ft) = frame_tracker().lock() {
                snap.frames = ft.frame_stats();
                snap.phases = ft.phase_stats();
                snap.allocs = ft.alloc_stats();
                snap.subsystems = ft.subsystem_stats();
                snap.jank_events = ft.jank_events();
            }

            {
                let prev_leak = metrics.lock().map(|s| s.leak.clone()).unwrap_or_default();
                snap.leak = prev_leak;
                snap.leak.update(snap.process.rss);
            }

            if let Ok(mut m) = metrics.lock() { *m = snap; }
        }
    }).expect("Failed to spawn monitoring thread");

    eprintln!("[monitoring] Profiling started — frame phases, alloc tracking, jank detection, GPU telemetry");
}
