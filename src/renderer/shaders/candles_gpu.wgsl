/**
 * GPU-resident candle shader — integer-pixel layout.
 *
 * All X positions are computed in integer physical pixels on the CPU and
 * passed as pre-rounded values.  The shader converts to clip space only at
 * the very end.  This guarantees:
 *   • Every bar body is exactly the same pixel count wide
 *   • Every gap between bodies is exactly the same pixel count wide
 *   • Every wick is exactly 1 physical pixel wide
 *   • No sub-pixel variation anywhere in the X axis
 *
 * Uniform layout (80 bytes, 16-byte aligned):
 *   [0]  viewStart(u32), viewCount(u32), _pad×2
 *   [16] stepPx(f32), bodyHalfPx(f32), priceA(f32), priceB(f32)
 *   [32] offsetPx(f32), _pad, canvasWidth(f32), canvasHeight(f32)
 *   [48] upColor   vec4
 *   [64] downColor vec4
 *
 *   stepPx      – bar slot width in physical pixels (integer, ≥ 1)
 *   bodyHalfPx  – half body width in physical pixels (integer, ≥ 1)
 *   offsetPx    – scroll offset in physical pixels (integer)
 *   priceA/B    – priceToClipY(p) = priceA + p * priceB  (one FMA, no branch)
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
  viewStart:    u32,
  viewCount:    u32,
  _pad0:        u32,
  _pad1:        u32,
  stepPx:       f32,   // bar slot width (integer physical pixels)
  bodyHalfPx:   f32,   // body half-width (integer physical pixels)
  priceA:       f32,
  priceB:       f32,
  offsetPx:     f32,   // scroll offset (integer physical pixels)
  _pad2:        f32,
  canvasWidth:  f32,   // physical canvas width  (CSS px × DPR)
  canvasHeight: f32,   // physical canvas height (CSS px × DPR)
  upColor:      vec4<f32>,
  downColor:    vec4<f32>,
}

@group(0) @binding(0) var<storage, read> bars: array<Bar>;
@group(0) @binding(1) var<uniform> vp: Viewport;

struct VertOut {
  @builtin(position)              pos:        vec4<f32>,
  @location(0)                    color:      vec4<f32>,
  @location(1) @interpolate(flat) bodyBounds: vec4<f32>,  // left, top, right, bottom (physical px)
  @location(2) @interpolate(flat) isBody:     u32,
}

// Physical pixel X → clip-space X
fn pxToClipX(px: f32) -> f32 { return px * 2.0 / vp.canvasWidth - 1.0; }

fn priceY(p: f32) -> f32 { return vp.priceA + p * vp.priceB; }

@vertex
fn vs_main(
  @builtin(vertex_index)   vi:   u32,
  @builtin(instance_index) inst: u32,
) -> VertOut {
  let barIdx = vp.viewStart + inst;

  if (barIdx >= arrayLength(&bars)) {
    var out: VertOut;
    out.pos        = vec4(-2.0, -2.0, 0.0, 1.0);
    out.color      = vec4(0.0);
    out.bodyBounds = vec4(0.0);
    out.isBody     = 0u;
    return out;
  }

  let bar = bars[barIdx];

  // ── Integer pixel X geometry ──────────────────────────────────────────────
  // All values are pre-rounded integers — no sub-pixel variation.
  let barLeftPx    = f32(inst) * vp.stepPx - vp.offsetPx;   // body left edge (integer px)
  let barRightPx   = barLeftPx + vp.bodyHalfPx * 2.0;        // body right edge (integer px)
  let wickCenterPx = barLeftPx + vp.bodyHalfPx;              // wick center (integer px)

  // Wick is exactly 1 physical pixel wide:
  //   rect [wickCenterPx, wickCenterPx + 1) covers the pixel at wickCenterPx
  let wickLP = pxToClipX(wickCenterPx);
  let wickRP = pxToClipX(wickCenterPx + 1.0);
  let bodyLP = pxToClipX(barLeftPx);
  let bodyRP = pxToClipX(barRightPx);

  // ── Price Y (floating point — smooth price animation) ─────────────────────
  let openY   = priceY(bar.open);
  let closeY  = priceY(bar.close);
  let highY   = priceY(bar.high);
  let lowY    = priceY(bar.low);
  let bodyTop = max(openY, closeY);
  let bodyBot = min(openY, closeY);

  let corners = array<vec2<f32>, 6>(
    vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
    vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
  );

  var minPt:      vec2<f32>;
  var maxPt:      vec2<f32>;
  var ci:         u32;
  var isBodyQuad: u32;

  if (vi < 6u) {
    ci         = vi;
    minPt      = vec2(bodyLP, bodyBot);
    maxPt      = vec2(bodyRP, bodyTop);
    isBodyQuad = 1u;
  } else if (vi < 12u) {
    ci         = vi - 6u;
    minPt      = vec2(wickLP, bodyTop);
    maxPt      = vec2(wickRP, highY);
    isBodyQuad = 0u;
  } else {
    ci         = vi - 12u;
    minPt      = vec2(wickLP, lowY);
    maxPt      = vec2(wickRP, bodyBot);
    isBodyQuad = 0u;
  }

  let c   = corners[ci];
  let pos = minPt + c * (maxPt - minPt);
  let col = select(vp.downColor, vp.upColor, bar.close >= bar.open);

  // Body pixel bounds for SDF rounded-corner in fragment shader.
  // X is in integer physical pixels; Y uses float clip→px conversion.
  let bTop = (1.0 - bodyTop) * 0.5 * vp.canvasHeight;
  let bBot = (1.0 - bodyBot) * 0.5 * vp.canvasHeight;

  var out: VertOut;
  out.pos        = vec4(pos, 0.0, 1.0);
  out.color      = col;
  out.bodyBounds = vec4(barLeftPx, bTop, barRightPx, bBot);
  out.isBody     = isBodyQuad;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  if (in.isBody == 1u) {
    let halfW  = (in.bodyBounds.z - in.bodyBounds.x) * 0.5;
    let halfH  = (in.bodyBounds.w - in.bodyBounds.y) * 0.5;
    let center = vec2(in.bodyBounds.x + halfW, in.bodyBounds.y + halfH);

    let r    = min(4.5, min(halfW, halfH));
    let d    = abs(in.pos.xy - center) - vec2(halfW, halfH) + vec2(r);
    let dist = length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - r;

    let alpha = 1.0 - smoothstep(-0.5, 0.5, dist);
    if (alpha < 0.002) { discard; }
    return vec4(in.color.rgb, in.color.a * alpha);
  }
  return in.color;
}
