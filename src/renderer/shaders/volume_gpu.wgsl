/**
 * GPU-resident volume shader.
 * Same storage buffer as candles_gpu.wgsl — only the 80-byte Viewport
 * uniform is rewritten when the viewport or maxVolume changes.
 *
 * Uniform layout (80 bytes, 16-byte aligned):
 *   [0]  viewStart, viewCount, _pad, _pad
 *   [16] barStepClip, pixelOffsetFrac, bodyWidthClip, maxVolume
 *   [32] volBottomClip, volHeightClip, _pad, _pad
 *   [48] upColor   vec4
 *   [64] downColor vec4
 */

struct Bar {
  open:   f32,
  high:   f32,
  low:    f32,
  close:  f32,
  volume: f32,
  _pad:   f32,
}

struct Viewport {
  viewStart:       u32,
  viewCount:       u32,
  _pad0:           u32,
  _pad1:           u32,
  barStepClip:     f32,
  pixelOffsetFrac: f32,
  bodyWidthClip:   f32,   // half-width of volume bar in clip space
  maxVolume:       f32,   // max volume in viewport (for normalisation)
  volBottomClip:   f32,   // clip Y of chart bottom (= -1.0)
  volHeightClip:   f32,   // max bar height in clip units (= 0.3)
  _pad2:           f32,
  _pad3:           f32,
  upColor:         vec4<f32>,
  downColor:       vec4<f32>,
}

@group(0) @binding(0) var<storage, read> bars: array<Bar>;
@group(0) @binding(1) var<uniform> vp: Viewport;

struct VertOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
}

fn barX(inst: u32) -> f32 {
  return vp.barStepClip * (f32(inst) + 0.5 - vp.pixelOffsetFrac) - 1.0;
}

@vertex
fn vs_main(
  @builtin(vertex_index)   vi:   u32,
  @builtin(instance_index) inst: u32,
) -> VertOut {
  let barIdx = vp.viewStart + inst;

  // Out-of-bounds guard
  if (barIdx >= arrayLength(&bars)) {
    var out: VertOut;
    out.pos   = vec4(-2.0, -2.0, 0.0, 1.0);
    out.color = vec4(0.0);
    return out;
  }

  let bar = bars[barIdx];

  let xc = barX(inst);
  let h  = (bar.volume / vp.maxVolume) * vp.volHeightClip;

  let corners = array<vec2<f32>, 6>(
    vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
    vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
  );

  let c      = corners[vi];
  let left   = xc - vp.bodyWidthClip;
  let right  = xc + vp.bodyWidthClip;
  let bottom = vp.volBottomClip;
  let top    = bottom + h;

  let x   = left   + c.x * (right  - left);
  let y   = bottom + c.y * (top    - bottom);
  let col = select(vp.downColor, vp.upColor, bar.close >= bar.open);

  var out: VertOut;
  out.pos   = vec4(x, y, 0.0, 1.0);
  out.color = col;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  return in.color;
}
