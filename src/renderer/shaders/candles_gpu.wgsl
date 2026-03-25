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
 * KEY: body half-width is rounded to the nearest integer physical pixel
 * before use so every bar is the same pixel count wide.  Without this,
 * at non-integer barStep sizes bars alternate between N and N+1 px wide,
 * which is the primary reason GPU-rendered charts feel visually "off"
 * compared to TradingView.
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
  bodyWidthClip:   f32,   // half-width of candle body in clip space (un-snapped)
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

  // ── Pixel-perfect body width ────────────────────────────────────────────
  // Convert clip-space half-width → physical pixels → round → back to clip.
  // This ensures ALL bars are the same integer pixel count wide, eliminating
  // the N / N+1 alternation that makes charts look "off" at non-integer scales.
  let rawHalfPx  = vp.bodyWidthClip * (vp.canvasWidth * 0.5);
  let snapHalfPx = max(1.0, round(rawHalfPx));
  let snapBodyHW = snapHalfPx / (vp.canvasWidth * 0.5);   // clip-space half-width

  // Unit quad corners — reused for body, upper wick, lower wick
  let corners = array<vec2<f32>, 6>(
    vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
    vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
  );

  var minPt:      vec2<f32>;
  var maxPt:      vec2<f32>;
  var ci:         u32;
  var isBodyQuad: u32;

  if (vi < 6u) {
    // Body — pixel-snapped width
    ci          = vi;
    minPt       = vec2(xc - snapBodyHW, bodyBot);
    maxPt       = vec2(xc + snapBodyHW, bodyTop);
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

  // Body bounding box in framebuffer pixels — uses snapped width so the SDF
  // radius is computed against the same geometry that the GPU rasterizes.
  let bLeft  = (xc - snapBodyHW + 1.0) * 0.5 * vp.canvasWidth;
  let bRight = (xc + snapBodyHW + 1.0) * 0.5 * vp.canvasWidth;
  let bTop   = (1.0 - bodyTop) * 0.5 * vp.canvasHeight;
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

    // 4.5px corner radius, capped so it never exceeds the smaller half-dimension
    let r = min(4.5, min(halfW, halfH));

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
