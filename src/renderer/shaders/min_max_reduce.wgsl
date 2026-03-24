/**
 * Parallel price range reduction.
 * Computes min(low) and max(high) over bars[viewStart..viewStart+viewCount).
 *
 * Positive f32 values preserve ordering when bitcast to u32 (IEEE 754),
 * so atomicMin/atomicMax on u32 gives correct results for prices.
 *
 * Result layout (8 bytes):
 *   [0] u32 = bitcast<u32>(minLow)   — caller initialises to 0xFFFFFFFF
 *   [1] u32 = bitcast<u32>(maxHigh)  — caller initialises to 0x00000000
 */

struct Bar {
  open:   f32,
  high:   f32,
  low:    f32,
  close:  f32,
  volume: f32,
  _pad:   f32,
}

struct Params {
  viewStart: u32,
  viewCount: u32,
  _pad0:     u32,
  _pad1:     u32,
}

struct Result {
  minLow:  atomic<u32>,
  maxHigh: atomic<u32>,
}

@group(0) @binding(0) var<storage, read>       bars:   array<Bar>;
@group(0) @binding(1) var<uniform>             params: Params;
@group(0) @binding(2) var<storage, read_write> result: Result;

var<workgroup> wgMin: atomic<u32>;
var<workgroup> wgMax: atomic<u32>;

@compute @workgroup_size(256)
fn cs_main(
  @builtin(global_invocation_id) gid: vec3<u32>,
  @builtin(local_invocation_id)  lid: vec3<u32>,
) {
  // Thread 0 initialises workgroup accumulators
  if (lid.x == 0u) {
    atomicStore(&wgMin, 0xFFFFFFFFu);
    atomicStore(&wgMax, 0u);
  }
  workgroupBarrier();

  let idx = gid.x;
  if (idx < params.viewCount) {
    let barIdx = params.viewStart + idx;
    if (barIdx < arrayLength(&bars)) {
      let bar = bars[barIdx];
      atomicMin(&wgMin, bitcast<u32>(bar.low));
      atomicMax(&wgMax, bitcast<u32>(bar.high));
    }
  }
  workgroupBarrier();

  // One thread per workgroup merges into global result
  if (lid.x == 0u) {
    atomicMin(&result.minLow,  atomicLoad(&wgMin));
    atomicMax(&result.maxHigh, atomicLoad(&wgMax));
  }
}
