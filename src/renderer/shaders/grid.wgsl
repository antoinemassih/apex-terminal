struct LineVert {
  @location(0) pos: vec2<f32>,
  @location(1) color: vec4<f32>,
}

struct VertOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(v: LineVert) -> VertOut {
  return VertOut(vec4(v.pos, 0.0, 1.0), v.color);
}

@fragment
fn fs_main(v: VertOut) -> @location(0) vec4<f32> {
  return v.color;
}
