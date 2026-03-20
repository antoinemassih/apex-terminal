struct CandleInstance {
  @location(0) x_clip:       f32,
  @location(1) open_clip:    f32,
  @location(2) close_clip:   f32,
  @location(3) low_clip:     f32,
  @location(4) high_clip:    f32,
  @location(5) body_w_clip:  f32,
  @location(6) color:        vec4<f32>,
}

struct Uniforms {
  wick_w_clip: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: CandleInstance) -> VertOut {
  let body_top    = max(inst.open_clip, inst.close_clip);
  let body_bottom = min(inst.open_clip, inst.close_clip);

  let corners = array<vec2<f32>, 6>(
    vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
    vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
  );

  var min_pt: vec2<f32>;
  var max_pt: vec2<f32>;
  var idx: u32;

  if (vi < 6u) {
    idx = vi;
    min_pt = vec2(inst.x_clip - inst.body_w_clip, body_bottom);
    max_pt = vec2(inst.x_clip + inst.body_w_clip, body_top);
  } else if (vi < 12u) {
    idx = vi - 6u;
    min_pt = vec2(inst.x_clip - u.wick_w_clip, body_top);
    max_pt = vec2(inst.x_clip + u.wick_w_clip, inst.high_clip);
  } else {
    idx = vi - 12u;
    min_pt = vec2(inst.x_clip - u.wick_w_clip, inst.low_clip);
    max_pt = vec2(inst.x_clip + u.wick_w_clip, body_bottom);
  }

  let c = corners[idx];
  let pos = min_pt + c * (max_pt - min_pt);

  var out: VertOut;
  out.pos = vec4(pos, 0.0, 1.0);
  out.color = inst.color;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  return in.color;
}
