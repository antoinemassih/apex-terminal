struct Uniforms {
  line_width: f32,
  _pad0: f32, _pad1: f32, _pad2: f32,
  color: vec4<f32>,
}
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) line_coord: vec2<f32>,
  @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(
  @builtin(vertex_index) vIdx: u32,
  @location(0) pointA: vec2<f32>,
  @location(1) pointB: vec2<f32>,
) -> VertOut {
  let corners = array<vec2<f32>, 6>(
    vec2(0.0, -0.5), vec2(1.0, -0.5), vec2(1.0, 0.5),
    vec2(0.0, -0.5), vec2(1.0,  0.5), vec2(0.0, 0.5),
  );
  let c = corners[vIdx];

  let dir = pointB - pointA;
  let len = length(dir);
  let xBasis = select(vec2(1.0, 0.0), dir / len, len > 0.0001);
  let yBasis = vec2(-xBasis.y, xBasis.x);

  let pos = pointA + xBasis * (c.x * len) + yBasis * (c.y * u.line_width);

  var out: VertOut;
  out.pos = vec4(pos, 0.0, 1.0);
  out.line_coord = c;
  out.color = u.color;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  let dist = abs(in.line_coord.y);
  let fw = fwidth(in.line_coord.y);
  let alpha = 1.0 - smoothstep(0.5 - fw, 0.5 + fw, dist);
  return vec4(in.color.rgb, in.color.a * alpha);
}
