/**
 * GPU-resident candle shader.
 * Bars live permanently in a storage buffer — only the 80-byte Viewport
 * uniform is rewritten on pan / zoom / tick.  Zero CPU coordinate work.
 *
 * Uniform layout (80 bytes, 16-byte aligned):
 *   [0]  viewStart, viewCount, _pad×2
 *   [16] barStepClip, pixelOffsetFrac, priceA, priceB
 *   [32] bodyWidthClip, wickWidthClip, canvasWidth, canvasHeight
 *   [48] upColor   vec4
 *   [64] downColor vec4
 *
 * priceToClipY(p) = priceA + p * priceB  (one FMA — no branch)
 *
 * Candle bodies have slightly rounded corners rendered via signed-distance-field
 * in the fragment shader.  Body bounding-box pixel coordinates are emitted from
 * the vertex shader as flat-interpolated outputs (constant per primitive) so the
 * fragment shader can compute the SDF without extra per-fragment uniforms.
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
  canvasWidth:     f32,   // physical canvas width  (CSS px × DPR)
  canvasHeight:    f32,   // physical canvas height (CSS px × DPR)
  upColor:         vec4<f32>,
  downColor:       vec4<f32>,
}

@group(0) @binding(0) var<storage, read> bars: array<Bar>;
@group(0) @binding(1) var<uniform> vp: Viewport;

struct VertOut {
  @builtin(position)              pos:        vec4<f32>,
  @location(0)                    color:      vec4<f32>,
  // Flat (constant per primitive): body bounding box in framebuffer pixels.
  // Used only when isBody == 1; wicks leave these as zero.
  @location(1) @interpolate(flat) bodyBounds: vec4<f32>,  // left, top, right, bottom
  @location(2) @interpolate(flat) isBody:     u32,
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
    out.pos        = vec4(-2.0, -2.0, 0.0, 1.0);
    out.color      = vec4(0.0);
    out.bodyBounds = vec4(0.0);
    out.isBody     = 0u;
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

  var minPt:      vec2<f32>;
  var maxPt:      vec2<f32>;
  var ci:         u32;
  var isBodyQuad: u32;

  if (vi < 6u) {
    // Body
    ci          = vi;
    minPt       = vec2(xc - vp.bodyWidthClip, bodyBot);
    maxPt       = vec2(xc + vp.bodyWidthClip, bodyTop);
    isBodyQuad  = 1u;
  } else if (vi < 12u) {
    // Upper wick
    ci          = vi - 6u;
    minPt       = vec2(xc - vp.wickWidthClip, bodyTop);
    maxPt       = vec2(xc + vp.wickWidthClip, highY);
    isBodyQuad  = 0u;
  } else {
    // Lower wick
    ci          = vi - 12u;
    minPt       = vec2(xc - vp.wickWidthClip, lowY);
    maxPt       = vec2(xc + vp.wickWidthClip, bodyBot);
    isBodyQuad  = 0u;
  }

  let c   = corners[ci];
  let pos = minPt + c * (maxPt - minPt);
  let col = select(vp.downColor, vp.upColor, bar.close >= bar.open);

  // Body bounding box in framebuffer pixels (for rounded-corner SDF in fs_main).
  // Clip → framebuffer: fb_x = (clip_x + 1) * 0.5 * W
  //                     fb_y = (1 - clip_y) * 0.5 * H   (y-axis is flipped)
  let bLeft  = (xc - vp.bodyWidthClip + 1.0) * 0.5 * vp.canvasWidth;
  let bRight = (xc + vp.bodyWidthClip + 1.0) * 0.5 * vp.canvasWidth;
  let bTop   = (1.0 - bodyTop) * 0.5 * vp.canvasHeight;  // higher clip Y → lower fb Y
  let bBot   = (1.0 - bodyBot) * 0.5 * vp.canvasHeight;

  var out: VertOut;
  out.pos        = vec4(pos, 0.0, 1.0);
  out.color      = col;
  out.bodyBounds = vec4(bLeft, bTop, bRight, bBot);
  out.isBody     = isBodyQuad;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  if (in.isBody == 1u) {
    let halfW  = (in.bodyBounds.z - in.bodyBounds.x) * 0.5;
    let halfH  = (in.bodyBounds.w - in.bodyBounds.y) * 0.5;
    let center = vec2(in.bodyBounds.x + halfW, in.bodyBounds.y + halfH);

    // Corner radius capped to the smaller half-dimension so it never dominates
    let r = min(3.0, min(halfW, halfH));

    // Signed distance to rounded rectangle
    let d    = abs(in.pos.xy - center) - vec2(halfW, halfH) + vec2(r);
    let dist = length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - r;

    // 1-pixel anti-aliased edge
    let alpha = 1.0 - smoothstep(-0.5, 0.5, dist);
    if (alpha < 0.002) { discard; }
    return vec4(in.color.rgb, in.color.a * alpha);
  }
  return in.color;
}
