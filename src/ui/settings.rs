//! Sidecar settings window. Pet animation/input logic does not live here;
//! this module only edits values and reports a patch for the app to apply.

use std::sync::Arc;
use std::time::Instant;

use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle, ViewportId};
use egui_wgpu::wgpu::{self, SurfaceTargetUnsafe};
use egui_wgpu::{Renderer, ScreenDescriptor};
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::config::{MAX_DISPLAY_SCALE, MIN_DISPLAY_SCALE};
use crate::locale::Locale;

#[derive(Clone, Copy)]
pub struct SettingsValues {
    pub display_scale: f32,
    pub look_range: u32,
    pub pointer_poll_ms: u64,
    pub loop_idle: bool,
    pub idle_animation: bool,
    pub always_on_top: bool,
}

#[derive(Default)]
pub struct SettingsPatch {
    pub display_scale: Option<f64>,
    pub look_range: Option<u32>,
    pub pointer_poll_ms: Option<u64>,
    pub loop_idle: Option<bool>,
    pub idle_animation: Option<bool>,
    pub always_on_top: Option<bool>,
    pub save: bool,
    pub close: bool,
}

/// Where the settings window should appear.
#[derive(Clone, Copy)]
pub enum SettingsAnchor {
    /// Center on the screen that contains this physical-pixel point (tray click).
    Point(i32, i32),
    /// Center on the screen that contains the pet window.
    Pet { x: i32, y: i32 },
}

pub struct SettingsWindow {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    ctx: egui::Context,
    winit_state: egui_winit::State,
    locale: Locale,
    pub values: SettingsValues,
    opened_at: Instant,
}

impl SettingsWindow {
    pub fn new(
        event_loop: &ActiveEventLoop,
        locale: Locale,
        values: SettingsValues,
        anchor: SettingsAnchor,
    ) -> Result<Self, crate::error::AppError> {
        let title = if locale == Locale::Zh {
            "CodingPet 设置"
        } else {
            "CodingPet Settings"
        };
        let inner = LogicalSize::new(440.0, 580.0);
        let estimated = estimated_outer_size(event_loop, anchor, inner);
        let (x, y) = centered_position(event_loop, anchor, estimated.0, estimated.1);
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(title)
                        .with_inner_size(inner)
                        .with_position(PhysicalPosition::new(x, y)),
                )
                .map_err(|e| crate::error::AppError::Window(e.to_string()))?,
        );

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = unsafe {
            instance
                .create_surface_unsafe(SurfaceTargetUnsafe::from_window(window.as_ref()).map_err(
                    |e| crate::error::AppError::Window(format!("settings surface: {e}")),
                )?)
                .map_err(|e| crate::error::AppError::Window(format!("settings surface: {e}")))?
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|e| crate::error::AppError::Window(format!("settings adapter: {e}")))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("settings"),
            required_features: wgpu::Features::empty(),
            required_limits:
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| crate::error::AppError::Window(format!("settings device: {e}")))?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let renderer = Renderer::new(&device, format, None, 1, false);
        let ctx = egui::Context::default();
        install_cjk_fonts(&ctx);
        apply_settings_theme(&ctx);
        let winit_state = egui_winit::State::new(
            ctx.clone(),
            ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        window.focus_window();
        window.request_redraw();

        Ok(Self {
            window,
            surface,
            device,
            queue,
            surface_config,
            renderer,
            ctx,
            winit_state,
            locale,
            values,
            opened_at: Instant::now(),
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn focus(&self) {
        self.window.focus_window();
        self.window.request_redraw();
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) -> SettingsPatch {
        let mut patch = SettingsPatch::default();
        match event {
            WindowEvent::CloseRequested => {
                // Drop the window and GPU resources only. Values were already
                // applied while the panel was open; closing must not rewrite fields.
                patch.close = true;
                return patch;
            }
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                self.surface_config.width = size.width;
                self.surface_config.height = size.height;
                self.surface.configure(&self.device, &self.surface_config);
                self.window.request_redraw();
            }
            WindowEvent::RedrawRequested => patch = self.draw(),
            _ => {}
        }
        if self
            .winit_state
            .on_window_event(&self.window, event)
            .repaint
        {
            self.window.request_redraw();
        }
        patch
    }

    fn draw(&mut self) -> SettingsPatch {
        let raw_input = self.winit_state.take_egui_input(&self.window);
        let mut patch = SettingsPatch::default();
        let locale = self.locale;
        let appear = appear_t(self.opened_at);
        let ctx = self.ctx.clone();
        let output = ctx.run(raw_input, |ctx| {
            patch = paint_ui(ctx, locale, &mut self.values, appear);
        });
        self.winit_state
            .handle_platform_output(&self.window, output.platform_output);

        let ppp = self.ctx.pixels_per_point();
        let primitives = self.ctx.tessellate(output.shapes, ppp);
        for (id, delta) in &output.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }
        let screen = ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: ppp,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("settings"),
            });
        let extra = self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &primitives,
            &screen,
        );
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                return patch;
            }
            Err(error) => {
                tracing::warn!("settings surface: {error}");
                return patch;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("settings"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.96,
                            g: 0.96,
                            b: 0.97,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), &primitives, &screen);
        }
        self.queue
            .submit(extra.into_iter().chain(std::iter::once(encoder.finish())));
        frame.present();
        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }
        if self.ctx.has_requested_repaint() || appear_t(self.opened_at) < 1.0 {
            self.window.request_redraw();
        }
        patch
    }
}

fn appear_t(opened_at: Instant) -> f32 {
    let t = (opened_at.elapsed().as_secs_f32() / 0.18).clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn estimated_outer_size(
    event_loop: &ActiveEventLoop,
    anchor: SettingsAnchor,
    inner: LogicalSize<f64>,
) -> (u32, u32) {
    let scale = monitor_for_anchor(event_loop, anchor)
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);
    (
        (inner.width * scale).round() as u32,
        (inner.height * scale + 36.0 * scale).round() as u32,
    )
}

fn monitor_for_anchor(
    event_loop: &ActiveEventLoop,
    anchor: SettingsAnchor,
) -> Option<winit::monitor::MonitorHandle> {
    let (ax, ay) = match anchor {
        SettingsAnchor::Point(x, y) | SettingsAnchor::Pet { x, y } => (x, y),
    };
    event_loop
        .available_monitors()
        .find(|monitor| {
            let pos = monitor.position();
            let size = monitor.size();
            ax >= pos.x
                && ay >= pos.y
                && ax < pos.x + size.width as i32
                && ay < pos.y + size.height as i32
        })
        .or_else(|| event_loop.primary_monitor())
        .or_else(|| event_loop.available_monitors().next())
}

fn centered_position(
    event_loop: &ActiveEventLoop,
    anchor: SettingsAnchor,
    win_w: u32,
    win_h: u32,
) -> (i32, i32) {
    let Some(monitor) = monitor_for_anchor(event_loop, anchor) else {
        return (0, 0);
    };
    let pos = monitor.position();
    let size = monitor.size();
    (
        pos.x + (size.width as i32 - win_w as i32) / 2,
        pos.y + (size.height as i32 - win_h as i32) / 2,
    )
}

fn apply_settings_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.window_fill = egui::Color32::from_rgb(245, 245, 247);
    visuals.panel_fill = egui::Color32::from_rgb(245, 245, 247);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(255, 255, 255);
    ctx.set_visuals(visuals);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(18);
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(20.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(12.0, egui::FontFamily::Proportional),
    );
    ctx.set_style(style);
}

fn paint_ui(
    ctx: &egui::Context,
    locale: Locale,
    values: &mut SettingsValues,
    appear: f32,
) -> SettingsPatch {
    let zh = locale == Locale::Zh;
    let mut patch = SettingsPatch::default();
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.set_opacity(appear);
        ui.add_space(8.0 * (1.0 - appear));
        ui.heading(if zh { "设置" } else { "Settings" });
        ui.add_space(4.0);
        ui.weak(if zh {
            "改动会立刻作用到宠物，关闭窗口不会再改配置。"
        } else {
            "Changes apply immediately. Closing does not rewrite settings."
        });
        ui.add_space(12.0);

        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(12))
            .corner_radius(8)
            .show(ui, |ui| {
                ui.label(egui::RichText::new(if zh { "外观" } else { "Appearance" }).strong());
                ui.add_space(6.0);
                ui.label(if zh { "宠物大小" } else { "Pet size" });
                let scale = ui.add(
                    egui::Slider::new(
                        &mut values.display_scale,
                        MIN_DISPLAY_SCALE as f32..=MAX_DISPLAY_SCALE as f32,
                    )
                    .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)),
                );
                ui.small(if zh {
                    "只改变显示尺寸，不会改精灵图本身。"
                } else {
                    "Changes on-screen size only."
                });
                if scale.changed() {
                    patch.display_scale = Some(f64::from(values.display_scale));
                }
                if scale.drag_stopped() {
                    patch.save = true;
                }

                ui.add_space(10.0);
                ui.label(if zh {
                    "感应范围"
                } else {
                    "Awareness range"
                });
                let range =
                    ui.add(egui::Slider::new(&mut values.look_range, 80..=600).suffix(" px"));
                ui.small(if zh {
                    "鼠标进入这个范围时，宠物会看向你。"
                } else {
                    "The pet looks toward the cursor inside this range."
                });
                if range.changed() {
                    patch.look_range = Some(values.look_range);
                }
                if range.drag_stopped() {
                    patch.save = true;
                }

                ui.add_space(10.0);
                ui.label(if zh {
                    "感应刷新间隔"
                } else {
                    "Awareness poll interval"
                });
                let poll = ui.add(
                    egui::Slider::new(
                        &mut values.pointer_poll_ms,
                        crate::config::MIN_POINTER_POLL_MS..=crate::config::MAX_POINTER_POLL_MS,
                    )
                    .suffix(" ms"),
                );
                ui.small(if zh {
                    "越低越跟手，越高轮询越少。"
                } else {
                    "Lower is snappier; higher polls less often."
                });
                if poll.changed() {
                    patch.pointer_poll_ms = Some(values.pointer_poll_ms);
                }
                if poll.drag_stopped() {
                    patch.save = true;
                }
            });

        ui.add_space(12.0);
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(12))
            .corner_radius(8)
            .show(ui, |ui| {
                ui.label(egui::RichText::new(if zh { "行为" } else { "Behavior" }).strong());
                ui.add_space(6.0);
                if ui
                    .checkbox(
                        &mut values.loop_idle,
                        if zh {
                            "循环待机动画"
                        } else {
                            "Loop idle animation"
                        },
                    )
                    .changed()
                {
                    patch.loop_idle = Some(values.loop_idle);
                    patch.save = true;
                }
                if ui
                    .checkbox(
                        &mut values.idle_animation,
                        if zh {
                            "随机闲置动作"
                        } else {
                            "Random idle actions"
                        },
                    )
                    .changed()
                {
                    patch.idle_animation = Some(values.idle_animation);
                    patch.save = true;
                }
                if ui
                    .checkbox(
                        &mut values.always_on_top,
                        if zh { "置顶" } else { "Always on top" },
                    )
                    .changed()
                {
                    patch.always_on_top = Some(values.always_on_top);
                    patch.save = true;
                }
            });
    });
    patch
}

fn install_cjk_fonts(ctx: &egui::Context) {
    let Some(bytes) = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
    ]
    .into_iter()
    .find_map(|path| std::fs::read(path).ok()) else {
        return;
    };
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".into(), Arc::new(FontData::from_owned(bytes)));
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "cjk".into());
    }
    ctx.set_fonts(fonts);
}
