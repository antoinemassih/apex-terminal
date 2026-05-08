// Phase 4b band-fill shader — instanced rendering, one quad per bar pair.
// Used for Bollinger Bands / Keltner Channels / Ichimoku cloud fills, where
// the area between an upper and a lower line gets a translucent tint.
// Sharing the candle pipeline's ViewUniform (same NDC transform).

struct ViewUniform {
    vs_frac: f32,
    vc_total: f32,
    price_low: f32,
    price_high: f32,
    chart_x_min: f32,
    chart_x_max: f32,
    chart_y_min: f32,
    chart_y_max: f32,
    body_half_slot: f32,
    wick_half_ndc: f32,
    surface_w: f32,
    surface_h: f32,
    bull: vec4<f32>,
    bear: vec4<f32>,
}

@group(0) @binding(0) var<uniform> view: ViewUniform;

struct FillIn {
    @location(0) start_slot: f32,
    @location(1) start_top:  f32,
    @location(2) start_bot:  f32,
    @location(3) end_slot:   f32,
    @location(4) end_top:    f32,
    @location(5) end_bot:    f32,
    @location(6) color:      vec4<f32>,
}

struct VertOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

fn slot_to_ndc_x(slot: f32) -> f32 {
    // Exact match for egui's bx(): rect.left() + (slot - 2*frac + 0.5)*bs.
    let slot_w = (view.chart_x_max - view.chart_x_min) / view.vc_total;
    return view.chart_x_min + (slot - 2.0 * view.vs_frac + 0.5) * slot_w;
}

fn price_to_ndc_y(price: f32) -> f32 {
    let range = view.price_high - view.price_low;
    let t = (price - view.price_low) / range;
    // No clamp — scissor handles off-chart clipping.
    return mix(view.chart_y_min, view.chart_y_max, t);
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, q: FillIn) -> VertOut {
    let x0 = slot_to_ndc_x(q.start_slot);
    let x1 = slot_to_ndc_x(q.end_slot);
    let y_top0 = price_to_ndc_y(q.start_top);
    let y_bot0 = price_to_ndc_y(q.start_bot);
    let y_top1 = price_to_ndc_y(q.end_top);
    let y_bot1 = price_to_ndc_y(q.end_bot);

    var pos: vec2<f32>;
    // 6 verts forming the band quad
    switch vid {
        case 0u: { pos = vec2<f32>(x0, y_top0); }
        case 1u: { pos = vec2<f32>(x1, y_top1); }
        case 2u: { pos = vec2<f32>(x0, y_bot0); }
        case 3u: { pos = vec2<f32>(x1, y_top1); }
        case 4u: { pos = vec2<f32>(x1, y_bot1); }
        case 5u: { pos = vec2<f32>(x0, y_bot0); }
        default: { pos = vec2<f32>(0.0, 0.0); }
    }

    var out: VertOut;
    out.pos = vec4<f32>(pos, 0.0, 1.0);
    out.color = q.color;
    return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    return in.color;
}
