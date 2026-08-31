use std::sync::Arc;

use egui::ViewportId;
use egui_wgpu::wgpu::{self, SurfaceTargetUnsafe};
use egui_wgpu::{Renderer, RendererOptions, ScreenDescriptor};
use winit::event::WindowEvent;
use winit::window::{Window, WindowId};

use crate::error::AppError;

/// On-demand egui/wgpu renderer owned by a sidecar window.
/// Dropping it releases the window's GPU resources with the window itself.
pub struct EguiWindowRenderer {
    window: Arc<Window>,
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    ctx: egui::Context,
    winit_state: egui_winit::State,
    label: &'static str,
}

impl EguiWindowRenderer {
    pub fn new(window: Arc<Window>, label: &'static str) -> Result<Self, AppError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(window.clone()),
        ));
        let surface = create_surface(&instance, &window, label)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|error| AppError::Window(format!("{label} adapter: {error}")))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some(label),
            required_features: wgpu::Features::empty(),
            required_limits:
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| AppError::Window(format!("{label} device: {error}")))?;

        let size = window.inner_size();
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(capabilities.formats[0]);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: capabilities
                .alpha_modes
                .iter()
                .copied()
                .find(|mode| {
                    matches!(
                        mode,
                        wgpu::CompositeAlphaMode::PreMultiplied
                            | wgpu::CompositeAlphaMode::PostMultiplied
                    )
                })
                .unwrap_or(capabilities.alpha_modes[0]),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let ctx = egui::Context::default();
        let winit_state = egui_winit::State::new(
            ctx.clone(),
            ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let renderer = Renderer::new(&device, format, RendererOptions::default());

        Ok(Self {
            window,
            instance,
            surface,
            device,
            queue,
            surface_config,
            renderer,
            ctx,
            winit_state,
            label,
        })
    }

    pub fn context(&self) -> &egui::Context {
        &self.ctx
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        if let WindowEvent::Resized(size) = event {
            if size.width > 0 && size.height > 0 {
                self.surface_config.width = size.width;
                self.surface_config.height = size.height;
                self.surface.configure(&self.device, &self.surface_config);
                self.window.request_redraw();
            }
        }
        if self
            .winit_state
            .on_window_event(&self.window, event)
            .repaint
        {
            self.window.request_redraw();
        }
    }

    pub fn draw(&mut self, mut paint: impl FnMut(&mut egui::Ui)) {
        let raw_input = self.winit_state.take_egui_input(&self.window);
        let output = self.ctx.run_ui(raw_input, |ui| paint(ui));
        self.winit_state
            .handle_platform_output(&self.window, output.platform_output);

        let pixels_per_point = self.ctx.pixels_per_point();
        let primitives = self.ctx.tessellate(output.shapes, pixels_per_point);
        for (id, delta) in &output.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }
        let screen = ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(self.label),
            });
        let extra_commands = self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &primitives,
            &screen,
        );

        let current = self.surface.get_current_texture();
        let mut reconfigure_after_present = false;
        let frame = match current {
            wgpu::CurrentSurfaceTexture::Success(frame) => Some(frame),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                reconfigure_after_present = true;
                Some(frame)
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_config);
                self.window.request_redraw();
                None
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                match create_surface(&self.instance, &self.window, self.label) {
                    Ok(surface) => {
                        self.surface = surface;
                        self.surface.configure(&self.device, &self.surface_config);
                        self.window.request_redraw();
                    }
                    Err(error) => tracing::warn!("{}", error),
                }
                None
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.window.request_redraw();
                None
            }
            wgpu::CurrentSurfaceTexture::Occluded => None,
            wgpu::CurrentSurfaceTexture::Validation => {
                tracing::warn!("{} surface validation failed", self.label);
                None
            }
        };

        if let Some(frame) = frame {
            let view = frame.texture.create_view(&Default::default());
            {
                let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(self.label),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                self.renderer
                    .render(&mut pass.forget_lifetime(), &primitives, &screen);
            }
            self.window.pre_present_notify();
            self.queue.submit(
                extra_commands
                    .into_iter()
                    .chain(std::iter::once(encoder.finish())),
            );
            frame.present();
            if reconfigure_after_present {
                self.surface.configure(&self.device, &self.surface_config);
                self.window.request_redraw();
            }
        }

        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }
        if self.ctx.has_requested_repaint() {
            self.window.request_redraw();
        }
    }
}

fn create_surface(
    instance: &wgpu::Instance,
    window: &Arc<Window>,
    label: &str,
) -> Result<wgpu::Surface<'static>, AppError> {
    unsafe {
        instance
            .create_surface_unsafe(
                SurfaceTargetUnsafe::from_window(window.as_ref())
                    .map_err(|error| AppError::Window(format!("{label} surface: {error}")))?,
            )
            .map_err(|error| AppError::Window(format!("{label} surface: {error}")))
    }
}
