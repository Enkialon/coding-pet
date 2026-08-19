use std::sync::Arc;

use objc2_app_kit::{NSView, NSWindowCollectionBehavior};
use winit::event_loop::ActiveEventLoop;
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowLevel};

use super::NativeWindowHandle;

#[allow(dead_code)]
pub struct PetWindow {
    pub window: Arc<Window>,
    pub width: u32,
    pub height: u32,
}

impl PetWindow {
    pub fn new(
        event_loop: &ActiveEventLoop,
        width: u32,
        height: u32,
        pos_x: i32,
        pos_y: i32,
    ) -> Result<Self, crate::error::AppError> {
        let attrs = winit::window::WindowAttributes::default()
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(true)
            .with_titlebar_hidden(true)
            .with_titlebar_buttons_hidden(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true)
            .with_movable_by_window_background(true)
            .with_has_shadow(false)
            .with_accepts_first_mouse(true);

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .map_err(|error| crate::error::AppError::Window(error.to_string()))?,
        );
        window.set_outer_position(winit::dpi::PhysicalPosition::new(pos_x, pos_y));

        let pet_window = Self {
            window,
            width,
            height,
        };
        pet_window.configure_spaces()?;
        Ok(pet_window)
    }

    /// Keep the pet visible when the user changes Spaces and alongside
    /// full-screen applications, without participating in Cmd+Tab cycling.
    fn configure_spaces(&self) -> Result<(), crate::error::AppError> {
        let NativeWindowHandle::AppKit(view) = self.native_handle()?;
        let view = unsafe { &*(view.cast::<NSView>()) };
        if let Some(window) = view.window() {
            window.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary
                    | NSWindowCollectionBehavior::IgnoresCycle,
            );
        }
        Ok(())
    }

    pub fn native_handle(&self) -> Result<NativeWindowHandle, crate::error::AppError> {
        let handle = self
            .window
            .window_handle()
            .map_err(|error| crate::error::AppError::Window(error.to_string()))?;
        match handle.as_raw() {
            RawWindowHandle::AppKit(handle) => {
                Ok(NativeWindowHandle::AppKit(handle.ns_view.as_ptr()))
            }
            _ => Err(crate::error::AppError::Window(
                "Not an AppKit window".into(),
            )),
        }
    }

    pub fn drag_window(&self) -> bool {
        self.window.drag_window().is_ok()
    }

    #[allow(dead_code)]
    pub fn set_outer_position(&self, x: i32, y: i32) {
        self.window
            .set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
    }

    pub fn outer_position(&self) -> (i32, i32) {
        self.window
            .outer_position()
            .map(|position| (position.x, position.y))
            .unwrap_or((0, 0))
    }

    #[allow(dead_code)]
    pub fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    pub fn set_always_on_top(&self, enabled: bool) {
        self.window.set_window_level(if enabled {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        });
    }

    pub fn is_position_on_any_monitor(x: i32, y: i32, event_loop: &ActiveEventLoop) -> bool {
        event_loop.available_monitors().any(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            x >= position.x
                && x < position.x + size.width as i32
                && y >= position.y
                && y < position.y + size.height as i32
        })
    }

    pub fn primary_monitor_center(event_loop: &ActiveEventLoop) -> (i32, i32) {
        event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .map(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                (
                    position.x + size.width as i32 / 2,
                    position.y + size.height as i32 / 2,
                )
            })
            .unwrap_or((0, 0))
    }
}
