struct VolumeInstance {
  @location(0) x_clip:    f32,
  @location(1) height:    f32, // 0-1 normalized
  @location(2) body_w:    f32,
  @location(3) color:     vec4<f32>,
}

struct VertOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: VolumeInstance) -> VertOut {
  // Volume bars sit at the bottom of the chart
  let corners = array<vec2<f32>, 6>(
    vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
    vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
  );

  let c = corners[vi];
  let left = inst.x_clip - inst.body_w;
  let right = inst.x_clip + inst.body_w;
  let bottom = -1.0; // bottom of clip space
  let top = bottom + inst.height * 0.3; // volume takes bottom 30% of chart

  let x = left + c.x * (right - left);
  let y = bottom + c.y * (top - bottom);

  var out: VertOut;
  out.pos = vec4(x, y, 0.0, 1.0);
  out.color = inst.color;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  return in.color;
}
