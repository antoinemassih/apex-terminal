// Phase 2 candle shader — instanced rendering, 12 vertices per candle.
// Vertices 0..6 produce the body quad; vertices 6..12 produce the wick quad.
// The chart render pass runs first (LoadOp::Clear), egui chrome paints on top.

struct ViewUniform {
    vs_frac: f32,         // fractional part of chart.vs (sub-bar pan offset)
    vc_total: f32,        // visible count + dynamic padding (bar slots across chart width)
    price_low: f32,       // min visible price (bottom of chart area)
    price_high: f32,      // max visible price (top of chart area)
    chart_x_min: f32,     // NDC x of chart area left edge
    chart_x_max: f32,     // NDC x of chart area right edge
    chart_y_min: f32,     // NDC y of chart area bottom (smaller value, low price)
    chart_y_max: f32,     // NDC y of chart area top (larger value, high price)
    body_half_slot: f32,  // body half-width as fraction of one bar slot (0.35)
    wick_half_ndc: f32,   // half-width of 1px wick in NDC = 1.0 / surface_width
    surface_w: f32,       // physical pixels — used by line shader
    surface_h: f32,       // physical pixels — used by line shader
    bull: vec4<f32>,      // bull candle RGBA
    bear: vec4<f32>,      // bear candle RGBA
}

@group(0) @binding(0) var<uniform> view: ViewUniform;

struct InstanceIn {
    @location(0) bar_slot: f32,  // 0-indexed from floor(chart.vs)
    @location(1) open: f32,
    @location(2) high: f32,
    @location(3) low: f32,
    @location(4) close: f32,
    @location(5) flags: u32,     // bit 0: bull (close >= open)
}

struct VertOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

fn price_to_ndc_y(price: f32) -> f32 {
    let range = view.price_high - view.price_low;
    let t = (price - view.price_low) / range;
    // t=0 → bottom of chart (chart_y_min), t=1 → top (chart_y_max)
    return mix(view.chart_y_min, view.chart_y_max, clamp(t, 0.0, 1.0));
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: InstanceIn) -> VertOut {
    let is_bull = (inst.flags & 1u) != 0u;

    // Horizontal geometry
    let slot_float = inst.bar_slot - view.vs_frac;
    let slot_w = (view.chart_x_max - view.chart_x_min) / view.vc_total;
    let x_center = view.chart_x_min + (slot_float + 0.5) * slot_w;
    let half_body = slot_w * view.body_half_slot;

    // Vertical geometry — bull: body from open (bot) to close (top)
    let body_top_price = select(inst.open, inst.close, is_bull);
    let body_bot_price = select(inst.close, inst.open, is_bull);

    let body_top = price_to_ndc_y(body_top_price);
    let body_bot = price_to_ndc_y(body_bot_price);
    let wick_top = price_to_ndc_y(inst.high);
    let wick_bot = price_to_ndc_y(inst.low);

    // Enforce 1-pixel minimum body height so doji candles remain visible
    let min_h = 2.0 * view.wick_half_ndc;
    let adj_body_top = max(body_top, body_bot + min_h);

    var out: VertOut;
    out.color = select(view.bear, view.bull, is_bull);

    if vid < 6u {
        // Body quad (two CW triangles covering the body rectangle)
        let verts = array<vec2<f32>, 6>(
            vec2<f32>(x_center - half_body, adj_body_top),
            vec2<f32>(x_center + half_body, adj_body_top),
            vec2<f32>(x_center - half_body, body_bot),
            vec2<f32>(x_center + half_body, adj_body_top),
            vec2<f32>(x_center + half_body, body_bot),
            vec2<f32>(x_center - half_body, body_bot),
        );
        out.pos = vec4<f32>(verts[vid], 0.0, 1.0);
    } else {
        // Wick quad — 1px wide vertical rect spanning high to low
        let wverts = array<vec2<f32>, 6>(
            vec2<f32>(x_center - view.wick_half_ndc, wick_top),
            vec2<f32>(x_center + view.wick_half_ndc, wick_top),
            vec2<f32>(x_center - view.wick_half_ndc, wick_bot),
            vec2<f32>(x_center + view.wick_half_ndc, wick_top),
            vec2<f32>(x_center + view.wick_half_ndc, wick_bot),
            vec2<f32>(x_center - view.wick_half_ndc, wick_bot),
        );
        out.pos = vec4<f32>(wverts[vid - 6u], 0.0, 1.0);
    }
    return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    return in.color;
}
