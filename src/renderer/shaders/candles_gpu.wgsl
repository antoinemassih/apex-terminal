/**
 * GPU-resident candle shader.
 * Bars live permanently in a storage buffer — only the 80-byte Viewport
 * uniform is rewritten on pan / zoom / tick.  Zero CPU coordinate work.
 *
 * Uniform layout (80 bytes, 16-byte aligned):
 *   [0] viewStart, viewCount, _pad, _pad
 *   [16] barStepClip, pixelOffsetFrac, priceA, priceB
 *   [32] bodyWidthClip, wickWidthClip, _pad, _pad
 *   [48] upColor   vec4
 *   [64] downColor vec4
 *
 * priceToClipY(p) = priceA + p * priceB  (one FMA — no branch)
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
  priceA:          f32,   // chartBottomClip - minPrice * priceB
  priceB:          f32,   // (chartTopClip - chartBottomClip) / (maxPrice - minPrice)
  bodyWidthClip:   f32,   // half-width of candle body in clip space
  wickWidthClip:   f32,   // half-width of wick in clip space
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

fn priceY(p: f32) -> f32 {
  return vp.priceA + p * vp.priceB;
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

  // Out-of-bounds guard — sends vertex off-screen rather than rendering garbage
  if (barIdx >= arrayLength(&bars)) {
    var out: VertOut;
    out.pos   = vec4(-2.0, -2.0, 0.0, 1.0);
    out.color = vec4(0.0);
    return out;
  }

  let bar = bars[barIdx];

  let xc      = barX(inst);
  let openY   = priceY(bar.open);
  let closeY  = priceY(bar.close);
  let highY   = priceY(bar.high);
  let lowY    = priceY(bar.low);
  let bodyTop = max(openY, closeY);
  let bodyBot = min(openY, closeY);

  // Unit quad corners — reused for all three rectangles
  let corners = array<vec2<f32>, 6>(
    vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
    vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
  );

  var minPt: vec2<f32>;
  var maxPt: vec2<f32>;
  var ci: u32;

  if (vi < 6u) {
    // Body
    ci = vi;
    minPt = vec2(xc - vp.bodyWidthClip, bodyBot);
    maxPt = vec2(xc + vp.bodyWidthClip, bodyTop);
  } else if (vi < 12u) {
    // Upper wick
    ci = vi - 6u;
    minPt = vec2(xc - vp.wickWidthClip, bodyTop);
    maxPt = vec2(xc + vp.wickWidthClip, highY);
  } else {
    // Lower wick
    ci = vi - 12u;
    minPt = vec2(xc - vp.wickWidthClip, lowY);
    maxPt = vec2(xc + vp.wickWidthClip, bodyBot);
  }

  let c   = corners[ci];
  let pos = minPt + c * (maxPt - minPt);
  let col = select(vp.downColor, vp.upColor, bar.close >= bar.open);

  var out: VertOut;
  out.pos   = vec4(pos, 0.0, 1.0);
  out.color = col;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  return in.color;
}
