/**
 * GPU-resident indicator line shader.
 *
 * Values live in a storage buffer permanently.  Per-frame CPU work:
 *   - tick update  : 4-byte writeBuffer
 *   - pan / zoom   : 64-byte viewport uniform write
 *   - new bar      : 4-byte writeBuffer
 *
 * NaN values in the buffer produce degenerate (off-screen) segments,
 * giving the same gap behaviour as the CPU NaN-skip path.
 *
 * Uniform layout (64 bytes, 16-byte aligned):
 *   [0]  viewStart (u32), segCount (u32), _pad, _pad
 *   [16] barStepClip, pixelOffsetFrac, priceA, priceB
 *   [32] lineWidthClip, _pad, _pad, _pad
 *   [48] color vec4
 */

// bitcast NaN detection — immune to float-point compiler optimisations
fn isNanF32(v: f32) -> bool {
  return (bitcast<u32>(v) & 0x7FFFFFFFu) > 0x7F800000u;
}

struct Viewport {
  viewStart:       u32,
  segCount:        u32,
  _pad0:           u32,
  _pad1:           u32,
  barStepClip:     f32,
  pixelOffsetFrac: f32,
  priceA:          f32,
  priceB:          f32,
  lineWidthClip:   f32,
  _pad2:           f32,
  _pad3:           f32,
  _pad4:           f32,
  color:           vec4<f32>,
}

@group(0) @binding(0) var<storage, read> values: array<f32>;
@group(0) @binding(1) var<uniform> vp: Viewport;

struct VertOut {
  @builtin(position) pos:       vec4<f32>,
  @location(0)       lineCoord: vec2<f32>,
  @location(1)       color:     vec4<f32>,
}

fn barX(localIdx: u32) -> f32 {
  return vp.barStepClip * (f32(localIdx) + 0.5 - vp.pixelOffsetFrac) - 1.0;
}

@vertex
fn vs_main(
  @builtin(vertex_index)   vi:   u32,
  @builtin(instance_index) inst: u32,
) -> VertOut {
  let idxA = vp.viewStart + inst;
  let idxB = idxA + 1u;
  let valA = values[idxA];
  let valB = values[idxB];

  // Collapse segment to off-screen if either endpoint is NaN (indicator gap)
  if (isNanF32(valA) || isNanF32(valB)) {
    var out: VertOut;
    out.pos       = vec4(-2.0, -2.0, 0.0, 1.0);
    out.lineCoord = vec2(0.0);
    out.color     = vp.color;
    return out;
  }

  let ptA = vec2(barX(inst),      vp.priceA + valA * vp.priceB);
  let ptB = vec2(barX(inst + 1u), vp.priceA + valB * vp.priceB);

  // Billboarded segment — same maths as legacy line.wgsl
  let corners = array<vec2<f32>, 6>(
    vec2(0.0, -0.5), vec2(1.0, -0.5), vec2(1.0,  0.5),
    vec2(0.0, -0.5), vec2(1.0,  0.5), vec2(0.0,  0.5),
  );
  let c    = corners[vi];
  let dir  = ptB - ptA;
  let len  = length(dir);
  let xBas = select(vec2(1.0, 0.0), dir / len, len > 0.0001);
  let yBas = vec2(-xBas.y, xBas.x);
  let pos  = ptA + xBas * (c.x * len) + yBas * (c.y * vp.lineWidthClip);

  var out: VertOut;
  out.pos       = vec4(pos, 0.0, 1.0);
  out.lineCoord = c;
  out.color     = vp.color;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  let dist  = abs(in.lineCoord.y);
  let fw    = fwidth(in.lineCoord.y);
  let alpha = 1.0 - smoothstep(0.5 - fw, 0.5 + fw, dist);
  return vec4(in.color.rgb, in.color.a * alpha);
}
