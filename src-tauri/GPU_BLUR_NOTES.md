# GPU Blur Notes — Future Work for `ui_kit::widgets::shadow`

The current `shadow::paint` (v1) draws N stacked rounded rectangles
with a cubic alpha falloff. This is fast and visually convincing up
to ~24px radius. Past that, layer seams become visible — particularly
on dark backgrounds and at extreme DPI scales. This note sketches the
true GPU path for v2.

## Strategy: two-pass separable Gaussian on a small offscreen texture

A 2D Gaussian blur is separable, so we can do two 1D passes (horizontal
then vertical) instead of one O(r²) pass. Cost is O(2r) per pixel.

### Pipeline sketch

1. **Render target sizing.** For a target rect `R` and radius `σ`,
   allocate (or reuse) an offscreen `wgpu::Texture` of size
   `(R.w + 6σ) × (R.h + 6σ)` rounded up to the next size bucket
   (powers of 2: 64, 128, 256, 512). Cap at 512×512 — anything larger
   should fall back to the v1 stacked-rect path.

2. **Pass 0 — silhouette.** Clear the texture with the target's
   shadow tint at zero alpha; draw a filled rounded rect (matching
   the panel's corner radius) into the centre at full alpha. This is
   the "silhouette" that gets blurred.

3. **Pass 1 — horizontal blur.** Bind the silhouette texture as input,
   render to a second ping-pong texture using a fragment shader that
   samples `2σ + 1` taps along the X axis with Gaussian weights
   precomputed in a uniform buffer.

4. **Pass 2 — vertical blur.** Same shader, sample along Y, render
   back to the first texture (or a third — ping-pong).

5. **Composite.** During egui's normal paint phase, register a
   `egui_wgpu::CallbackTrait` that issues one textured-quad draw of
   the blurred texture at `target_rect.translate(offset).expand(3σ)`,
   modulated by `spec.color`.

### Shader sketch (WGSL, horizontal pass)

```wgsl
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> weights: array<vec4<f32>, 17>; // up to 65 taps

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let dim = vec2<f32>(textureDimensions(src));
    let dx = 1.0 / dim.x;
    var acc = vec4<f32>(0.0);
    for (var i: i32 = -32; i <= 32; i = i + 1) {
        let w = weights[(i + 32) / 4][(i + 32) % 4];
        acc = acc + w * textureSample(src, samp, uv + vec2<f32>(f32(i) * dx, 0.0));
    }
    return acc;
}
```

### Texture pool design

Allocating a fresh `wgpu::Texture` every frame is the perf footgun.
We keep a `ShadowTexturePool` keyed on size bucket:

```rust
struct ShadowTexturePool {
    // bucket size -> ring of (texture, view, last_used_frame) entries.
    buckets: HashMap<u32, Vec<TextureSlot>>,
    current_frame: u64,
}
```

- Acquire: pop an idle slot from the bucket, or allocate.
- Release: marked free at end-of-frame.
- GC: drop slots untouched for >120 frames (~2s @ 60fps).

Stored in `egui_wgpu::CallbackResources` (the per-RenderState type
map) so it survives across frames and is shared with the chart
renderer's existing wgpu resources.

### Where the device lives

`egui::Context::request_repaint` is paint-phase; the wgpu device is
reachable inside a `CallbackTrait::prepare` impl via the
`&egui_wgpu::ScreenDescriptor` and `&wgpu::Device` parameters. Look
at `src/chart/renderer/gpu.rs` for an existing example of how the
chart paints custom wgpu content via egui paint callbacks.

### Performance budget

- Allocate at most one 512×512 RGBA8 = 1MB per concurrent shadow.
- Two render passes at 512×512 with 33 taps each ≈ 17M sampling ops,
  trivially <0.2ms on any GPU we'd ship to.
- Aim: total shadow cost <0.5ms/frame for up to 4 concurrent
  shadows (typical max: modal + tooltip + 2 popovers).

### Migration

`ShadowSpec` stays unchanged. Add a `shadow::set_backend(Backend::Gpu)`
or auto-pick based on radius: stacked-rect for <=24px, GPU two-pass
for >24px. Callers don't change.
