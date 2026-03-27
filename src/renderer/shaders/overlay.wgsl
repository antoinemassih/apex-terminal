/**
 * GPU overlay line shader — renders arbitrary line segments with per-line color.
 *
 * Used for: crosshair lines, trendlines, hlines, hzone borders, selection outlines.
 * Each line is a billboarded anti-aliased quad (6 vertices per instance).
 *
 * Storage buffer layout per line: 12 f32s (48 bytes)
 *   [0..1]  start position (clip space x, y)
 *   [2..3]  end position (clip space x, y)
 *   [4..7]  color (rgba)
 *   [8]     dash pattern: 0 = solid, >0 = dash length in clip units
 *   [9]     gap length in clip units (if dashed)
 *   [10]    line width in clip units
 *   [11]    _pad
 */

struct Line {
  x0: f32, y0: f32,
  x1: f32, y1: f32,
  r: f32, g: f32, b: f32, a: f32,
  dashLen: f32,
  gapLen: f32,
  width: f32,
  _pad: f32,
}

@group(0) @binding(0) var<storage, read> lines: array<Line>;

struct VertOut {
  @builtin(position) pos:   vec4<f32>,
  @location(0) lineCoord:   vec2<f32>,  // x = along line [0..1], y = across [-0.5..0.5]
  @location(1) color:       vec4<f32>,
  @location(2) lineLen:     f32,        // total length in clip units (for dash calc)
  @location(3) dashLen:     f32,
  @location(4) gapLen:      f32,
}

@vertex
fn vs_main(
  @builtin(vertex_index)   vi:   u32,
  @builtin(instance_index) inst: u32,
) -> VertOut {
  let ln = lines[inst];
  let ptA = vec2(ln.x0, ln.y0);
  let ptB = vec2(ln.x1, ln.y1);

  let corners = array<vec2<f32>, 6>(
    vec2(0.0, -0.5), vec2(1.0, -0.5), vec2(1.0,  0.5),
    vec2(0.0, -0.5), vec2(1.0,  0.5), vec2(0.0,  0.5),
  );
  let c   = corners[vi];
  let dir = ptB - ptA;
  let len = length(dir);
  let xBas = select(vec2(1.0, 0.0), dir / len, len > 0.0001);
  let yBas = vec2(-xBas.y, xBas.x);
  let pos  = ptA + xBas * (c.x * len) + yBas * (c.y * ln.width);

  var out: VertOut;
  out.pos       = vec4(pos, 0.0, 1.0);
  out.lineCoord = c;
  out.color     = vec4(ln.r, ln.g, ln.b, ln.a);
  out.lineLen   = len;
  out.dashLen   = ln.dashLen;
  out.gapLen    = ln.gapLen;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  // Anti-aliased edge
  let dist  = abs(in.lineCoord.y);
  let fw    = fwidth(in.lineCoord.y);
  var alpha = 1.0 - smoothstep(0.5 - fw, 0.5 + fw, dist);

  // Dashed line pattern
  if (in.dashLen > 0.0) {
    let period = in.dashLen + in.gapLen;
    let along  = in.lineCoord.x * in.lineLen;
    let phase  = along % period;
    let edge   = fwidth(along) * 1.5;
    // Smooth transition at dash edges
    alpha *= smoothstep(in.dashLen + edge, in.dashLen - edge, phase);
  }

  return vec4(in.color.rgb, in.color.a * alpha);
}
