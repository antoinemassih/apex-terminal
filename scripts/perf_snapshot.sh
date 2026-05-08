#!/usr/bin/env bash
# perf_snapshot.sh — one-screen frame summary from the live :9091 endpoint.
# Used as a baseline tool for the GPU chart refactor (SPEC_GPU_CHART_REFACTOR.md).
# Run before/after each phase; paste output into the commit message.

set -u
HOST="${APEX_METRICS_HOST:-localhost:9091}"

raw=$(curl -fsS --max-time 3 "http://${HOST}/metrics" 2>/dev/null || true)
if [ -z "$raw" ]; then
  echo "ERROR: could not reach http://${HOST}/metrics — is the app running?" >&2
  exit 1
fi

# Pull a metric value by name. Returns "—" if absent (old binary missing
# Phase 0+ metrics, etc.).
g() {
  local name="$1"
  local v
  v=$(echo "$raw" | awk -v n="$name" '$1 == n { print $2; exit }')
  echo "${v:-—}"
}
# Pull a labeled gauge by metric + first label match.
gl() {
  local name="$1"
  local v
  v=$(echo "$raw" | awk -v n="$name" '
    $0 !~ /^#/ && index($0, n"{") == 1 { for (i=NF; i>=1; i--) { if ($i ~ /^[0-9.]+$/) { print $i; exit } } }
  ')
  echo "${v:-—}"
}

now=$(date "+%Y-%m-%d %H:%M:%S")
git_head=$(git -C "$(dirname "$0")/.." rev-parse --short HEAD 2>/dev/null || echo "?")
git_dirty=""
if ! git -C "$(dirname "$0")/.." diff --quiet 2>/dev/null; then git_dirty=" (dirty)"; fi

cat <<EOF
─── apex perf snapshot — ${now} — ${git_head}${git_dirty} ───

Frames
  fps avg ............... $(g apex_frame_fps)
  frame avg ............. $(g apex_frame_time_avg_us) us
  frame p99 ............. $(g apex_frame_time_p99_us) us
  frame max ............. $(g apex_frame_time_max_us) us
  frame min ............. $(g apex_frame_time_min_us) us
  total frames .......... $(g apex_frame_total)
  dropped (>33ms) ....... $(g apex_frame_dropped_total)
  jank events (>20ms) ... $(g apex_jank_events_total)

Phases (avg us)
  acquire ............... $(g apex_phase_acquire_avg_us)
  layout ................ $(g apex_phase_layout_avg_us)   max $(g apex_phase_layout_max_us)
  tessellate ............ $(g apex_phase_tessellate_avg_us)
  upload ................ $(g apex_phase_upload_avg_us)
  render ................ $(g apex_phase_render_avg_us)   max $(g apex_phase_render_max_us)
  present ............... $(g apex_phase_present_avg_us)
  chart_pass (gpu) ...... $(g apex_phase_chart_pass_avg_us)   max $(g apex_phase_chart_pass_max_us)

Chart pipeline
  active (0=egui 1=gpu) . $(g apex_chart_pipeline_active)
  visible bars .......... $(g apex_chart_visible_bars)

Render
  paint jobs / frame .... $(g apex_render_paint_jobs)
  vertices / frame ...... $(g apex_render_vertices)
  indices / frame ....... $(g apex_render_indices)

Allocations (per frame)
  count avg ............. $(g apex_alloc_frame_avg_count)
  bytes avg ............. $(g apex_alloc_frame_avg_bytes)
  count last ............ $(g apex_alloc_frame_count)
  bytes last ............ $(g apex_alloc_frame_bytes)
  net held .............. $(g apex_alloc_net_bytes)

Process
  rss ................... $(g apex_process_rss_bytes)
  cpu % ................. $(g apex_process_cpu_percent)
  uptime s .............. $(g apex_uptime_seconds)

GPU (active = first listed)
  utilization % ......... $(gl apex_gpu_utilization_percent)
  mem used .............. $(gl apex_gpu_memory_used_bytes)
  power W ............... $(gl apex_gpu_power_watts)
  graphics MHz .......... $(gl apex_gpu_clock_graphics_mhz)

Subsystems (avg us, sorted by avg desc)
$(echo "$raw" | awk '
  $0 ~ /^apex_subsystem_avg_us\{name=/ {
    match($0, /name="[^"]*"/); name = substr($0, RSTART+6, RLENGTH-7);
    avg = $2 + 0;
    printf "%-32s %6d us\n", name, avg;
  }
' | sort -k2 -n -r)
EOF
