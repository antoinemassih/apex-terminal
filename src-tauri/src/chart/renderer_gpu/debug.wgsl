// Phase 1 debug shader — paints a magenta rectangle covering the center 60%
// of the surface (NDC -0.6..0.6) to verify the chart render pass interleaves
// correctly with egui's render pass. Uses alpha blending so egui chrome
// remains visible underneath the tint.
//
// Replaced in Phase 2 by the real candle shader.

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(-0.6, -0.6),
        vec2<f32>( 0.6, -0.6),
        vec2<f32>(-0.6,  0.6),
        vec2<f32>( 0.6, -0.6),
        vec2<f32>( 0.6,  0.6),
        vec2<f32>(-0.6,  0.6),
    );
    return vec4<f32>(pos[vid], 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 1.0, 0.4);
}
