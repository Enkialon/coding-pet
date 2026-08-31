//! Sidecar settings window. Pet animation/input logic does not live here;
//! this module only edits values and reports a patch for the app to apply.

use std::sync::Arc;
use std::time::Instant;

use egui::{Align, Layout, Sense};
use egui_components::{Badge, Card, Heading, IconKind, Label, Slider, Switch, Variant};
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use super::renderer::EguiWindowRenderer;
use super::theme;
use crate::config::{MAX_DISPLAY_SCALE, MIN_DISPLAY_SCALE};
use crate::locale::{settings_strings, Locale};

const WINDOW_WIDTH: f64 = 480.0;
const WINDOW_HEIGHT: f64 = 620.0;
const WINDOW_CONTROLS_HEIGHT: f32 = 36.0;
const WINDOW_CORNER_RADIUS: f32 = 16.0;

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
    drag_window: bool,
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
    renderer: EguiWindowRenderer,
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
        let strings = settings_strings(locale);
        let inner = LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT);
        let estimated = estimated_window_size(event_loop, anchor, inner);
        let (x, y) = centered_position(event_loop, anchor, estimated.0, estimated.1);
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(strings.window_title)
                        .with_inner_size(inner)
                        .with_min_inner_size(inner)
                        .with_max_inner_size(inner)
                        .with_resizable(false)
                        .with_decorations(false)
                        .with_transparent(true)
                        .with_position(PhysicalPosition::new(x, y)),
                )
                .map_err(|error| crate::error::AppError::Window(error.to_string()))?,
        );
        let renderer = EguiWindowRenderer::new(window, "settings")?;
        theme::install(renderer.context());
        renderer.window().focus_window();
        renderer.window().request_redraw();

        Ok(Self {
            renderer,
            locale,
            values,
            opened_at: Instant::now(),
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.renderer.window_id()
    }

    pub fn focus(&self) {
        self.renderer.window().focus_window();
        self.renderer.window().request_redraw();
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) -> SettingsPatch {
        if matches!(event, WindowEvent::CloseRequested) {
            return SettingsPatch {
                close: true,
                ..Default::default()
            };
        }
        self.renderer.on_window_event(event);
        if matches!(event, WindowEvent::RedrawRequested) {
            return self.draw();
        }
        SettingsPatch::default()
    }

    fn draw(&mut self) -> SettingsPatch {
        let locale = self.locale;
        let appear = appear_t(self.opened_at);
        let values = &mut self.values;
        let mut patch = SettingsPatch::default();
        self.renderer.draw(|ui| {
            patch = paint_ui(ui, locale, values, appear);
        });
        if patch.drag_window {
            if let Err(error) = self.renderer.window().drag_window() {
                tracing::warn!("Could not drag settings window: {error}");
            }
        }
        if appear_t(self.opened_at) < 1.0 {
            self.renderer.window().request_redraw();
        }
        patch
    }
}

fn appear_t(opened_at: Instant) -> f32 {
    let t = (opened_at.elapsed().as_secs_f32() / 0.18).clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn paint_ui(
    ui: &mut egui::Ui,
    locale: Locale,
    values: &mut SettingsValues,
    appear: f32,
) -> SettingsPatch {
    let strings = settings_strings(locale);
    let mut patch = SettingsPatch::default();
    ui.painter().rect_filled(
        ui.max_rect(),
        egui::CornerRadius::same(WINDOW_CORNER_RADIUS as u8),
        theme::PAGE_BACKGROUND,
    );

    let (close, drag_window) = paint_window_controls(ui);
    patch.close = close;
    patch.drag_window = drag_window;

    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: 20,
            right: 20,
            top: 0,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_opacity(appear);
            ui.add_space(8.0 * (1.0 - appear));
            ui.add(Heading::new(strings.page_title).description(strings.page_description));
            ui.add_space(10.0);

            Card::new()
                .title(strings.appearance)
                .padding(12.0)
                .show(ui, |ui| {
                    slider_row(
                        ui,
                        strings.pet_size,
                        strings.pet_size_description,
                        format!("{:.0}%", values.display_scale * 100.0),
                        &mut values.display_scale,
                        MIN_DISPLAY_SCALE as f32..=MAX_DISPLAY_SCALE as f32,
                        |response, value, patch| {
                            if response.changed() {
                                patch.display_scale = Some(f64::from(value));
                            }
                        },
                        &mut patch,
                    );
                    ui.add_space(6.0);

                    let mut look_range = values.look_range as f32;
                    slider_row(
                        ui,
                        strings.awareness_range,
                        strings.awareness_range_description,
                        format!("{} px", values.look_range),
                        &mut look_range,
                        80.0..=600.0,
                        |response, value, patch| {
                            if response.changed() {
                                let value = value.round() as u32;
                                values.look_range = value;
                                patch.look_range = Some(value);
                            }
                        },
                        &mut patch,
                    );
                    ui.add_space(6.0);

                    let mut pointer_poll = values.pointer_poll_ms as f32;
                    slider_row(
                        ui,
                        strings.poll_interval,
                        strings.poll_interval_description,
                        format!("{} ms", values.pointer_poll_ms),
                        &mut pointer_poll,
                        crate::config::MIN_POINTER_POLL_MS as f32
                            ..=crate::config::MAX_POINTER_POLL_MS as f32,
                        |response, value, patch| {
                            if response.changed() {
                                let value = value.round() as u64;
                                values.pointer_poll_ms = value;
                                patch.pointer_poll_ms = Some(value);
                            }
                        },
                        &mut patch,
                    );
                });

            ui.add_space(10.0);
            Card::new()
                .title(strings.behavior)
                .padding(12.0)
                .show(ui, |ui| {
                    if switch_row(
                        ui,
                        strings.loop_idle,
                        strings.loop_idle_description,
                        &mut values.loop_idle,
                    ) {
                        patch.loop_idle = Some(values.loop_idle);
                        patch.save = true;
                    }
                    ui.add_space(6.0);
                    if switch_row(
                        ui,
                        strings.idle_animation,
                        strings.idle_animation_description,
                        &mut values.idle_animation,
                    ) {
                        patch.idle_animation = Some(values.idle_animation);
                        patch.save = true;
                    }
                    ui.add_space(6.0);
                    if switch_row(
                        ui,
                        strings.always_on_top,
                        strings.always_on_top_description,
                        &mut values.always_on_top,
                    ) {
                        patch.always_on_top = Some(values.always_on_top);
                        patch.save = true;
                    }
                });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui_components::Icon::new(IconKind::Heart)
                        .size(13.0)
                        .color(theme::ACCENT),
                );
                ui.add(Label::new(strings.auto_save).muted());
            });
        });
    patch
}

fn paint_window_controls(ui: &mut egui::Ui) -> (bool, bool) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), WINDOW_CONTROLS_HEIGHT),
        Sense::hover(),
    );
    let close_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 20.0, rect.center().y),
        egui::vec2(32.0, 28.0),
    );
    let drag_rect =
        egui::Rect::from_min_max(rect.min, egui::pos2(close_rect.left(), rect.bottom()));
    let drag = ui.interact(
        drag_rect,
        ui.id().with("settings-drag"),
        Sense::click_and_drag(),
    );
    let close = ui.interact(close_rect, ui.id().with("settings-close"), Sense::click());

    if close.hovered() {
        ui.painter().rect_filled(
            close_rect,
            egui::CornerRadius::same(8),
            egui::Color32::from_rgb(245, 245, 247),
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    egui_components::icon::paint_icon(
        ui.painter(),
        IconKind::Close,
        close_rect.shrink(8.0),
        if close.hovered() {
            egui::Color32::from_rgb(196, 57, 82)
        } else {
            theme::TEXT_MUTED
        },
        1.6,
    );

    (close.clicked(), drag.drag_started())
}

#[allow(clippy::too_many_arguments)]
fn slider_row(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    value_label: String,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    mut changed: impl FnMut(&egui::Response, f32, &mut SettingsPatch),
    patch: &mut SettingsPatch,
) {
    ui.horizontal(|ui| {
        ui.add(Label::new(title).strong())
            .on_hover_text(description);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add(Badge::new(value_label).variant(Variant::Secondary));
        });
    });
    ui.add_space(2.0);
    let width = ui.available_width();
    let response = ui.add(Slider::new(value, range).width(width));
    changed(&response, *value, patch);
    if response.drag_stopped() {
        patch.save = true;
    }
}

fn switch_row(ui: &mut egui::Ui, title: &str, description: &str, value: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add(Label::new(title).strong())
            .on_hover_text(description);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = ui.add(Switch::new(value)).changed();
        });
    });
    changed
}

fn estimated_window_size(
    event_loop: &ActiveEventLoop,
    anchor: SettingsAnchor,
    inner: LogicalSize<f64>,
) -> (u32, u32) {
    let scale = monitor_for_anchor(event_loop, anchor)
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);
    (
        (inner.width * scale).round() as u32,
        (inner.height * scale).round() as u32,
    )
}

fn monitor_for_anchor(
    event_loop: &ActiveEventLoop,
    anchor: SettingsAnchor,
) -> Option<winit::monitor::MonitorHandle> {
    let (anchor_x, anchor_y) = match anchor {
        SettingsAnchor::Point(x, y) | SettingsAnchor::Pet { x, y } => (x, y),
    };
    event_loop
        .available_monitors()
        .find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            anchor_x >= position.x
                && anchor_y >= position.y
                && anchor_x < position.x + size.width as i32
                && anchor_y < position.y + size.height as i32
        })
        .or_else(|| event_loop.primary_monitor())
        .or_else(|| event_loop.available_monitors().next())
}

fn centered_position(
    event_loop: &ActiveEventLoop,
    anchor: SettingsAnchor,
    window_width: u32,
    window_height: u32,
) -> (i32, i32) {
    let Some(monitor) = monitor_for_anchor(event_loop, anchor) else {
        return (0, 0);
    };
    let position = monitor.position();
    let size = monitor.size();
    (
        position.x + (size.width as i32 - window_width as i32) / 2,
        position.y + (size.height as i32 - window_height as i32) / 2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_window_contains_the_full_settings_page() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32),
            )),
            ..Default::default()
        };
        for locale in [Locale::Zh, Locale::En] {
            let mut used_bottom = 0.0;
            let mut values = SettingsValues {
                display_scale: 1.0,
                look_range: 240,
                pointer_poll_ms: 100,
                loop_idle: true,
                idle_animation: true,
                always_on_top: false,
            };

            let _ = ctx.run_ui(input.clone(), |ui| {
                let _ = paint_ui(ui, locale, &mut values, 1.0);
                used_bottom = ui.min_rect().bottom();
            });

            assert!(
                used_bottom <= WINDOW_HEIGHT as f32,
                "settings content ends at {used_bottom}, beyond the {WINDOW_HEIGHT}px window"
            );
        }
    }
}
