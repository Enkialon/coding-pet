use std::time::Instant;

use crate::config::Config;
use crate::integration::{AgentEventKind, AppUserEvent};
use crate::locale::Locale;
use crate::pet::model::PetAnimation;
use crate::platform::{PetWindow, Renderer};
use crate::ui::tray::{check_menu_event, PetTray};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::WindowId;

use super::{normalize_cursor_position, PetApp, WIN_HEIGHT, WIN_WIDTH};

impl ApplicationHandler<AppUserEvent> for PetApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return; // already initialised
        }

        // Determine initial window position: config or default centre
        let config = Config::load();
        let (pos_x, pos_y) = if config.position.x != 0 || config.position.y != 0 {
            // Validate saved position is still on a monitor
            if PetWindow::is_position_on_any_monitor(
                config.position.x,
                config.position.y,
                event_loop,
            ) {
                (config.position.x, config.position.y)
            } else {
                tracing::warn!(
                    "Saved position ({}, {}) is off-screen — using monitor centre",
                    config.position.x,
                    config.position.y
                );
                PetWindow::primary_monitor_center(event_loop)
            }
        } else {
            // First run: centre on primary monitor
            PetWindow::primary_monitor_center(event_loop)
        };

        let (out_w, out_h) = self.output_size();
        let win = match PetWindow::new(event_loop, out_w, out_h, pos_x, pos_y) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        // Windows draws into an on-screen DIB, so the buffer is the scaled
        // window. macOS keeps a 192×208 buffer and lets the surface scale it.
        let (buffer_w, buffer_h) = if cfg!(target_os = "macos") {
            (WIN_WIDTH, WIN_HEIGHT)
        } else {
            (out_w, out_h)
        };
        let mut renderer = match Renderer::new(&win, buffer_w, buffer_h) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to create renderer: {e}");
                event_loop.exit();
                return;
            }
        };

        // Build and start the Idle animation
        let idle_anim = self.idle_animation();
        self.animator.set_animation(PetAnimation::Idle, &idle_anim);
        if !self.loop_idle {
            self.animator.pause();
        }

        // Draw the first frame immediately
        let rect = self.animator.current_rect();
        let (pixels, stride) = self.active_spritesheet();
        renderer.draw_frame(pixels, stride, rect);

        // Apply always_on_top from config on startup
        if !config.always_on_top {
            win.set_always_on_top(false);
        }

        // Create the system tray
        let pet_names = Self::discover_pet_names();
        let locale = Locale::detect(&config.language);
        match PetTray::new(&pet_names, locale) {
            Ok(mut tray) => {
                tray.set_always_on_top_text(config.always_on_top);
                tracing::info!("System tray created with {} pet(s)", pet_names.len());
                self.tray = Some(tray);
            }
            Err(e) => {
                tracing::warn!("Failed to create system tray: {e}");
            }
        }

        let source = if self.provider.is_some() {
            "Codex pet"
        } else {
            "test fallback"
        };
        tracing::info!("Window and renderer initialised — animation started ({source})");
        self.window = Some(win);
        self.renderer = Some(renderer);
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            let now = Instant::now();

            self.update_pointer_awareness(now);

            // Tick the animation
            if self.animator.tick(now) {
                // Frame changed — request a redraw
                if let Some(win) = &self.window {
                    win.window.request_redraw();
                }
            }

            // Check if a one-shot animation just finished — return to idle
            if !self.animator.is_playing() && self.animator.is_finished() {
                self.restore_idle();
            }

            // Check ambient timer: trigger random one-shot animation when idle
            if self.visible && !self.pointer.is_held() && !self.looking {
                if let Some(anim) = self.ambient_timer.check(now) {
                    // Only trigger if currently in Idle animation
                    if self.animator.current_animation() == PetAnimation::Idle {
                        self.play_animation(anim);
                    }
                }
            }
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self
            .settings
            .as_ref()
            .is_some_and(|window| window.window_id() == window_id)
        {
            if let Some(settings) = self.settings.as_mut() {
                let patch = settings.on_window_event(&event);
                self.apply_settings_patch(patch);
            }
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                // The tray menu is the single place that explicitly exits or
                // hides the pet. Ignore OS close requests (for example Alt+F4)
                // so the desktop pet and tray icon remain visible.
                tracing::info!("Close requested — keeping desktop pet running");
            }
            WindowEvent::RedrawRequested => {
                let rect = self.animator.current_rect();
                let (pixels, stride) = if let Some(provider) = &self.provider {
                    (provider.spritesheet_pixels(), provider.spritesheet_stride())
                } else {
                    (self.test_spritesheet.as_slice(), self.test_stride)
                };
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.draw_frame(pixels, stride, rect);
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize_surface(size.width, size.height);
                }
                if let Some(window) = &self.window {
                    window.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let size = self
                    .window
                    .as_ref()
                    .map(|window| window.window.inner_size())
                    .unwrap_or(winit::dpi::PhysicalSize::new(WIN_WIDTH, WIN_HEIGHT));
                (self.cursor_x, self.cursor_y) =
                    normalize_cursor_position(position.x, position.y, size.width, size.height);
                self.sync_pointer_gesture();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.begin_pointer_press();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.end_pointer_gesture();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                // Show context menu at cursor position
                if let (Some(tray), Some(win)) = (&self.tray, &self.window) {
                    if let Ok(handle) = win.native_handle() {
                        let size = win.window.inner_size();
                        let x = self.cursor_x * size.width as i32 / WIN_WIDTH as i32;
                        let y = self.cursor_y * size.height as i32 / WIN_HEIGHT as i32;
                        tray.show_context_menu(handle, x, y);
                    }
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppUserEvent) {
        match event {
            AppUserEvent::Agent(event) => {
                tracing::info!(
                    source = %event.source,
                    event = %event.event,
                    session_id = event.session_id.as_deref().unwrap_or(""),
                    "Received coding-tool event"
                );
                let interacting = self.pointer.is_held() || self.looking;
                let animation = match event.kind() {
                    // Idle returns to the configured idle mode (loop or freeze).
                    AgentEventKind::Idle => {
                        self.base_animation = PetAnimation::Idle;
                        None
                    }
                    AgentEventKind::Working => {
                        self.base_animation = PetAnimation::Running;
                        Some(PetAnimation::Running)
                    }
                    AgentEventKind::Waiting => {
                        self.base_animation = PetAnimation::Waiting;
                        Some(PetAnimation::Waiting)
                    }
                    AgentEventKind::Reviewing => {
                        self.base_animation = PetAnimation::Idle;
                        Some(PetAnimation::Review)
                    }
                    AgentEventKind::Succeeded => {
                        self.base_animation = PetAnimation::Idle;
                        Some(PetAnimation::Waving)
                    }
                    AgentEventKind::Failed => {
                        self.base_animation = PetAnimation::Idle;
                        Some(PetAnimation::Failed)
                    }
                    AgentEventKind::Unknown => None,
                };
                if interacting {
                    self.pending_animation = animation;
                } else {
                    if let Some(animation) = animation {
                        self.play_animation(animation);
                    } else if event.kind() == AgentEventKind::Idle {
                        self.restore_idle();
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Process tray/context menu events
        while let Some(menu_event) = check_menu_event() {
            self.handle_menu_event(&menu_event, event_loop);
            if event_loop.exiting() {
                return;
            }
        }

        // Calculate the earliest wake-up time from animation and interaction timers.
        let anim_time = if self.animator.is_playing() {
            Some(self.animator.next_frame_time())
        } else {
            None
        };
        let ambient_time = if self.ambient_timer.is_enabled()
            && self.visible
            && !self.pointer.is_held()
            && !self.looking
        {
            Some(self.ambient_timer.next_trigger_time())
        } else {
            None
        };
        let pointer_time = if self.visible {
            Some(self.next_pointer_poll)
        } else {
            None
        };

        let next_wake = [anim_time, ambient_time, pointer_time]
            .into_iter()
            .flatten()
            .min();

        if let Some(deadline) = next_wake {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}
