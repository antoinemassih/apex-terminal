pub mod state;
/// W3-01: indicator trait + registry. Deliberately outside `renderer::gpu` —
/// this is the domain-extraction direction W4-01 will continue.
pub mod indicators;
pub mod renderer;
pub mod renderer_gpu;
