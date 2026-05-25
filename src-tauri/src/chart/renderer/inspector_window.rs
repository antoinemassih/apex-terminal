//! Inspector window — a second OS window for the F12 design inspector.
//!
//! Built as a separate winit::Window + wgpu::Surface + egui::Context so
//! it can be dragged to a second monitor. Shares the chart window's
//! wgpu::Device + wgpu::Queue (cheap Arc-clone in wgpu 24+) so we don't
//! spin up a second adapter.
//!
//! Why a separate egui::Context (and not egui's viewport API): egui's
//! viewport API requires the host (our `App` in `gpu.rs`) to implement
//! viewport-creation plumbing. That works in eframe but not in custom
//! winit + egui-wgpu integrations without significant additional code.
//! Going with a second standalone Context is simpler and gets us the
//! same user-visible result (real OS window on a separate monitor).
//!
//! State sharing: the inspector mutates `DesignTokens` via the existing
//! global RwLock (`crate::design_tokens`). The chart window reads from
//! that same RwLock in its `begin_frame()`. So slider edits in the
//! inspector window propagate to the chart window next frame — no
//! extra wiring needed.
//!
//! Lifecycle:
//! - User clicks POP on F12 inspector → `request_open()` is called
//!   (flips an AtomicBool).
//! - App's `about_to_wait` sees the request, creates this struct, and
//!   stores it on `App.inspector_window`.
//! - Every `RedrawRequested` event for this window calls `render()`.
//! - User closes the OS window → `CloseRequested` removes the struct
//!   from App + clears `Inspector::is_popout`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

// ─── Global request flags ────────────────────────────────────────────────────
//
// The inspector code (deep in the egui render loop) flips these to ask the
// App to create / destroy the inspector window. The App polls them in its
// `about_to_wait` hook where it has access to `&ActiveEventLoop` (needed
// to create windows).

static OPEN_REQUEST: AtomicBool = AtomicBool::new(false);
static CLOSE_REQUEST: AtomicBool = AtomicBool::new(false);

/// Called by the inspector when the user clicks POP. The App handles
/// the actual window creation on the next `about_to_wait` tick.
pub fn request_open() {
    OPEN_REQUEST.store(true, Ordering::Release);
}

/// Called by the inspector when the user clicks DOCK or otherwise wants
/// to send the window away. The App closes it on the next tick.
pub fn request_close() {
    CLOSE_REQUEST.store(true, Ordering::Release);
}

/// True iff an open request is pending. App polls + clears via `take_open_request`.
pub fn take_open_request() -> bool {
    OPEN_REQUEST.swap(false, Ordering::AcqRel)
}

/// True iff a close request is pending. App polls + clears via `take_close_request`.
pub fn take_close_request() -> bool {
    CLOSE_REQUEST.swap(false, Ordering::AcqRel)
}

// ─── InspectorWindow ─────────────────────────────────────────────────────────

pub struct InspectorWindow {
    pub id: WindowId,
    pub win: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub egui_renderer: egui_wgpu::Renderer,
    /// Set true on `CloseRequested`. App drops the struct next tick.
    pub close_requested: bool,
}

impl InspectorWindow {
    /// Create the window. Takes references to the chart window's device,
    /// queue, and adapter so we share GPU resources rather than booting a
    /// second adapter (wasteful + double-VRAM on multi-GPU machines).
    pub fn create(
        el: &ActiveEventLoop,
        device: wgpu::Device,
        queue: wgpu::Queue,
        instance: &wgpu::Instance,
        adapter: &wgpu::Adapter,
    ) -> Option<Self> {
        let attrs = WindowAttributes::default()
            .with_title("Apex Design Inspector")
            .with_inner_size(winit::dpi::LogicalSize::new(500.0, 800.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(360.0, 480.0))
            .with_resizable(true);

        let win = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("[inspector_window] failed to create OS window: {e}");
                return None;
            }
        };

        let size = win.inner_size();
        let surface = match unsafe {
            // wgpu 24's `create_surface_unsafe` lets us bind a non-'static
            // surface to a window we own via Arc<Window>; we then promote
            // it to 'static by holding the Arc on the same struct.
            instance.create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::from_window(&*win)
                    .expect("Window handle invalid"),
            )
        } {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[inspector_window] surface creation failed: {e}");
                return None;
            }
        };

        let caps = surface.get_capabilities(adapter);
        let fmt = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            wgpu::PresentMode::AutoVsync
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: fmt,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Separate egui::Context for this window. Apply same visuals as
        // the chart window so they look consistent.
        let egui_ctx = egui::Context::default();
        let mut visuals = egui::Visuals::dark();
        let r3 = egui::CornerRadius::same(4);
        let r6 = egui::CornerRadius::same(6);
        visuals.window_corner_radius = r6;
        visuals.menu_corner_radius = r3;
        visuals.widgets.noninteractive.corner_radius = r3;
        visuals.widgets.inactive.corner_radius = r3;
        visuals.widgets.hovered.corner_radius = r3;
        visuals.widgets.active.corner_radius = r3;
        visuals.widgets.open.corner_radius = r3;
        egui_ctx.set_visuals(visuals);
        crate::ui_kit::icons::init_icons(&egui_ctx);

        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &*win,
            Some(win.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(&device, fmt, None, 1, false);

        let id = win.id();
        win.request_redraw();

        Some(Self {
            id,
            win,
            surface,
            config,
            device,
            queue,
            egui_ctx,
            egui_state,
            egui_renderer,
            close_requested: false,
        })
    }

    /// Forward a winit WindowEvent to egui. Caller handles CloseRequested
    /// and Resized separately because those mutate App-level state.
    pub fn on_window_event(&mut self, ev: &winit::event::WindowEvent) -> egui_winit::EventResponse {
        self.egui_state.on_window_event(&self.win, ev)
    }

    /// Reconfigure the surface when the window resizes.
    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }

    /// Paint one frame. Runs the egui inspector body inside our Context,
    /// tessellates, renders via egui_wgpu, and presents.
    pub fn render(&mut self) {
        let raw_input = self.egui_state.take_egui_input(&self.win);
        // P6.1 — inspector bg now derives from the active theme (was a
        // hardcoded dark `from_rgb(14,14,20)` which was jarring on light themes).
        let theme = crate::chart_renderer::theme_impl::active_theme(&self.egui_ctx);
        let bg = theme.bg;
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(bg))
                .show(ctx, |ui| {
                    render_inspector_body_into(ui);
                });
        });

        self.egui_state
            .handle_platform_output(&self.win, full_output.platform_output);

        let pixels_per_point = self.egui_ctx.pixels_per_point();
        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, pixels_per_point);

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                // Surface invalidated (resize race). Try to reconfigure & skip
                // this frame; next redraw will succeed.
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("inspector_window_encoder"),
            });

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        };

        // Upload texture deltas.
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        {
            let mut rpass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("inspector_window_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.055,
                                g: 0.055,
                                b: 0.078,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                })
                .forget_lifetime();
            self.egui_renderer
                .render(&mut rpass, &paint_jobs, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        // Egui may request a repaint (animations, hovers). Honour it.
        if full_output.viewport_output.values().any(|v| {
            v.repaint_delay == std::time::Duration::ZERO
        }) {
            self.win.request_redraw();
        }
    }
}

/// Render the inspector body into the given egui::Ui. Pulled out so the
/// chart window (docked mode) and this window (popped-out mode) share
/// the same paint code.
fn render_inspector_body_into(ui: &mut egui::Ui) {
    super::gpu::DESIGN_INSPECTOR.with(|cell| {
        let mut cell = cell.borrow_mut();
        if let Some(insp) = cell.as_mut() {
            if let Some(mut tokens_local) = crate::design_tokens::get() {
                let mut modified = false;
                insp.show_inspector_body(ui, &mut tokens_local, &mut modified);
                if modified {
                    crate::design_tokens::update(tokens_local);
                }
            }
        }
    });
}
