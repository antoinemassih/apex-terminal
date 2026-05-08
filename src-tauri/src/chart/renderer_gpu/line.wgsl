// Phase 4a indicator line shader — instanced rendering, one segment per instance.
// Each segment draws a 6-vertex thick-line quad: two triangles expanded
// perpendicular to the segment direction by `thickness/2` physical pixels.
// Sharing the candle pipeline's ViewUniform binding (same NDC transform).

struct ViewUniform {
    vs_frac: f32,
    vc_total: f32,
    price_low: f32,
    price_high: f32,
    chart_x_min: f32,
    chart_x_max: f32,
    chart_y_min: f32,
    chart_y_max: f32,
    body_half_slot: f32,  // unused here
    wick_half_ndc: f32,   // unused here
    surface_w: f32,
    surface_h: f32,
    bull: vec4<f32>,      // unused here
    bear: vec4<f32>,      // unused here
}

@group(0) @binding(0) var<uniform> view: ViewUniform;

struct LineIn {
    @location(0) start_slot: f32,
    @location(1) start_val:  f32,
    @location(2) end_slot:   f32,
    @location(3) end_val:    f32,
    @location(4) color:      vec4<f32>,
    @location(5) thickness:  f32,    // physical pixels — full width
}

struct VertOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

fn slot_to_ndc_x(slot: f32) -> f32 {
    let slot_w = (view.chart_x_max - view.chart_x_min) / view.vc_total;
    let visible_slot = slot - view.vs_frac;
    return view.chart_x_min + (visible_slot + 0.5) * slot_w;
}

fn price_to_ndc_y(price: f32) -> f32 {
    let range = view.price_high - view.price_low;
    let t = (price - view.price_low) / range;
    return mix(view.chart_y_min, view.chart_y_max, clamp(t, 0.0, 1.0));
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, seg: LineIn) -> VertOut {
    let p0 = vec2<f32>(slot_to_ndc_x(seg.start_slot), price_to_ndc_y(seg.start_val));
    let p1 = vec2<f32>(slot_to_ndc_x(seg.end_slot),   price_to_ndc_y(seg.end_val));

    // Convert NDC delta to pixel delta to compute a screen-space perpendicular,
    // then convert the perpendicular offset back to NDC. This makes the on-screen
    // line thickness independent of the segment's slope and aspect ratio.
    let dx_ndc = p1.x - p0.x;
    let dy_ndc = p1.y - p0.y;
    let dx_px = dx_ndc * 0.5 * view.surface_w;
    let dy_px = -dy_ndc * 0.5 * view.surface_h;  // NDC Y flipped vs pixel Y
    let len_px = max(sqrt(dx_px * dx_px + dy_px * dy_px), 0.0001);
    let half_t = max(seg.thickness, 0.5) * 0.5;
    // Perpendicular in pixel space (rotate dir by 90°): (-dy, dx)
    let off_px = vec2<f32>(-dy_px, dx_px) * (half_t / len_px);
    // Back to NDC delta
    let off_ndc = vec2<f32>(
        off_px.x * 2.0 / view.surface_w,
        -off_px.y * 2.0 / view.surface_h,
    );

    var pos: vec2<f32>;
    // 6 verts forming a quad: (p0+off, p1+off, p0-off) + (p1+off, p1-off, p0-off)
    switch vid {
        case 0u: { pos = p0 + off_ndc; }
        case 1u: { pos = p1 + off_ndc; }
        case 2u: { pos = p0 - off_ndc; }
        case 3u: { pos = p1 + off_ndc; }
        case 4u: { pos = p1 - off_ndc; }
        case 5u: { pos = p0 - off_ndc; }
        default: { pos = vec2<f32>(0.0, 0.0); }
    }

    var out: VertOut;
    out.pos = vec4<f32>(pos, 0.0, 1.0);
    out.color = seg.color;
    return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    return in.color;
}
