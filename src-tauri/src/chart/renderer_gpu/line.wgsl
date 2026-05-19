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
    eth_alpha: f32,       // unused here
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0) var<uniform> view: ViewUniform;

struct LineIn {
    @location(0) start_slot: f32,
    @location(1) start_val:  f32,
    @location(2) end_slot:   f32,
    @location(3) end_val:    f32,
    @location(4) color:      vec4<f32>,
    @location(5) thickness:  f32,        // physical pixels — full width
    @location(6) dash_period_px: f32,    // 0 = solid; >0 = stipple period (px)
    @location(7) dash_duty:      f32,    // 0..1 = "on" fraction of period
}

struct VertOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    // Signed perpendicular distance from the line centre, in physical pixels.
    // Used by the fragment shader to feather the outer edges for AA.
    @location(1) dist_px: f32,
    // The line's true half-thickness in physical pixels. Constant for an
    // instance, passed flat so the fragment shader knows where the
    // alpha-fall-off band starts.
    @location(2) half_t: f32,
    // Distance along the line from start (p0) toward end (p1), in physical
    // pixels. Interpolates linearly from 0 at p0 to len_px at p1. Used by
    // the fragment shader for dash pattern modulation.
    @location(3) along_px: f32,
    // Dash period in physical pixels (constant for an instance). 0.0 = solid.
    @location(4) dash_period_px: f32,
    // Dash duty cycle (0..1) — fraction of `dash_period_px` that is "on".
    @location(5) dash_duty: f32,
}

fn slot_to_ndc_x(slot: f32) -> f32 {
    // Exact match for egui's bx(): rect.left() + (slot - 2*frac + 0.5)*bs.
    // Keeps GPU lines aligned with egui-rendered axes/handles/hit-tests.
    let slot_w = (view.chart_x_max - view.chart_x_min) / view.vc_total;
    return view.chart_x_min + (slot - 2.0 * view.vs_frac + 0.5) * slot_w;
}

fn price_to_ndc_y(price: f32) -> f32 {
    let range = view.price_high - view.price_low;
    let t = (price - view.price_low) / range;
    // No clamp — scissor rect handles clipping. Clamping made line endpoints
    // outside the visible price range snap to the chart top/bottom, skewing
    // line slopes during pan as one endpoint stuck at the edge.
    return mix(view.chart_y_min, view.chart_y_max, t);
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

    // Anti-aliasing: expand the quad by AA_FEATHER pixels on each side and
    // let the fragment shader fade alpha across the outer band. Without this,
    // diagonal lines render as solid stair-stepped quads (egui's line
    // tessellator does this feathering by default; we have to do it manually).
    let aa_feather = 1.0;
    let half_t_aa = half_t + aa_feather;
    // Perpendicular in pixel space (rotate dir by 90°): (-dy, dx)
    let off_px = vec2<f32>(-dy_px, dx_px) * (half_t_aa / len_px);
    // Back to NDC delta
    let off_ndc = vec2<f32>(
        off_px.x * 2.0 / view.surface_w,
        -off_px.y * 2.0 / view.surface_h,
    );

    var pos: vec2<f32>;
    var edge_sign: f32 = 0.0;
    // `along_t` is 0 at the p0 end and 1 at the p1 end. Verts 0,1,3 are
    // at the +off side; verts 2,4,5 are at the -off side; within each side
    // half the verts sit at p0 (along_t=0) and the other half at p1 (=1).
    var along_t: f32 = 0.0;
    // 6 verts forming a quad: (p0+off, p1+off, p0-off) + (p1+off, p1-off, p0-off)
    switch vid {
        case 0u: { pos = p0 + off_ndc; edge_sign =  1.0; along_t = 0.0; }
        case 1u: { pos = p1 + off_ndc; edge_sign =  1.0; along_t = 1.0; }
        case 2u: { pos = p0 - off_ndc; edge_sign = -1.0; along_t = 0.0; }
        case 3u: { pos = p1 + off_ndc; edge_sign =  1.0; along_t = 1.0; }
        case 4u: { pos = p1 - off_ndc; edge_sign = -1.0; along_t = 1.0; }
        case 5u: { pos = p0 - off_ndc; edge_sign = -1.0; along_t = 0.0; }
        default: { pos = vec2<f32>(0.0, 0.0); }
    }

    var out: VertOut;
    out.pos = vec4<f32>(pos, 0.0, 1.0);
    out.color = seg.color;
    // edge_sign is ±1 at the AA-expanded edge; interpolation gives 0 at centre.
    // Multiplied by half_t_aa gives signed pixel distance from line centre.
    out.dist_px = edge_sign * half_t_aa;
    out.half_t = half_t;
    // Linearly interpolate along the line: 0 px at p0, len_px at p1. The
    // fragment shader uses this to drive the dash pattern.
    out.along_px = along_t * len_px;
    out.dash_period_px = seg.dash_period_px;
    out.dash_duty = seg.dash_duty;
    return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    // Edge AA: smooth alpha falloff in the 1-pixel band outside the line's
    // "true" edge. Inside the line proper (|d| <= half_t - 0.5): full alpha.
    // Outside the AA band (|d| >= half_t + 0.5): zero alpha (clipped).
    // In between: smoothstep to feather the edge so diagonal lines don't
    // look stair-stepped.
    let abs_d = abs(in.dist_px);
    let edge_alpha = 1.0 - smoothstep(in.half_t - 0.5, in.half_t + 0.5, abs_d);

    // Dash pattern: when dash_period_px > 0, modulate alpha along the line
    // by a square wave of duty `dash_duty`. Sub-pixel feathering at the
    // dash/gap transitions prevents shimmering during pan. When period == 0
    // (solid), this collapses to a no-op (multiplier stays 1.0).
    var dash_alpha: f32 = 1.0;
    if (in.dash_period_px > 0.5) {
        let phase = (in.along_px / in.dash_period_px) - floor(in.along_px / in.dash_period_px); // fract()
        // Anti-alias the dash transitions: smoothstep across one pixel of
        // phase rather than a hard step. Pixel→phase conversion uses
        // 1/period so the AA band is exactly 1 physical pixel wide at the
        // start and end of the "on" segment.
        let aa_px = 1.0 / in.dash_period_px;
        let on_in  = smoothstep(0.0, aa_px, phase);
        let on_out = 1.0 - smoothstep(in.dash_duty - aa_px, in.dash_duty, phase);
        dash_alpha = on_in * on_out;
    }

    return vec4<f32>(in.color.rgb, in.color.a * edge_alpha * dash_alpha);
}
