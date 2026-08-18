// Window management — Phase 5
// Creates a borderless, always-on-top transparent window for the desktop pet.
// Transparency is handled entirely via UpdateLayeredWindow in the renderer,
// so we do NOT set winit's `with_transparent(true)`.
// Adds drag support, multi-monitor position validation, and DPI queries.

use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE,
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
};

#[allow(dead_code)]
pub struct PetWindow {
    pub window: Window,
    pub hwnd: isize,
    pub width: u32,
    pub height: u32,
}

impl PetWindow {
    /// Create a new borderless pet window at the given screen position.
    pub fn new(
        event_loop: &winit::event_loop::ActiveEventLoop,
        width: u32,
        height: u32,
        pos_x: i32,
        pos_y: i32,
    ) -> Result<Self, crate::error::AppError> {
        let attrs = winit::window::WindowAttributes::default()
            .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
            .with_decorations(false)
            // Note: We do NOT use with_transparent(true) because transparency
            // is managed by UpdateLayeredWindow + WS_EX_LAYERED in the renderer.
            .with_visible(true);

        let window = event_loop
            .create_window(attrs)
            .map_err(|e| crate::error::AppError::Window(e.to_string()))?;

        // Position the window on screen
        window.set_outer_position(winit::dpi::PhysicalPosition::new(pos_x, pos_y));

        // Extract the native HWND
        let handle = window
            .window_handle()
            .map_err(|e| crate::error::AppError::Window(e.to_string()))?;
        let hwnd = match handle.as_raw() {
            RawWindowHandle::Win32(h) => h.hwnd.get() as isize,
            _ => return Err(crate::error::AppError::Window("Not a Win32 window".into())),
        };

        // Apply extended window styles: layered + always-on-top + tool window
        // Also remove WS_THICKFRAME to prevent Aero Snap (screen-edge maximise).
        unsafe {
            // Remove resizable frame styles — prevents DWM Aero Snap
            let style = GetWindowLongPtrW(hwnd as _, GWL_STYLE);
            let new_style = style as usize
                & !(WS_THICKFRAME as usize | WS_MINIMIZEBOX as usize | WS_MAXIMIZEBOX as usize);
            SetWindowLongPtrW(hwnd as _, GWL_STYLE, new_style as _);

            let ex_style = GetWindowLongPtrW(hwnd as _, GWL_EXSTYLE);
            let new_ex = ex_style as usize
                | WS_EX_LAYERED as usize
                | WS_EX_TOPMOST as usize
                | WS_EX_TOOLWINDOW as usize;
            SetWindowLongPtrW(hwnd as _, GWL_EXSTYLE, new_ex as _);
            // SetWindowLongPtrW alone doesn't change Z-order — must call SetWindowPos
            SetWindowPos(
                hwnd as _,
                HWND_TOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }

        Ok(PetWindow {
            window,
            hwnd,
            width,
            height,
        })
    }

    /// Initiate a window drag (allows the user to move the pet by clicking and dragging).
    pub fn drag_window(&self) {
        let _ = self.window.drag_window();
    }

    /// Set the outer position of the window in physical pixels.
    #[allow(dead_code)]
    pub fn set_outer_position(&self, x: i32, y: i32) {
        self.window
            .set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
    }

    /// Get the current outer position of the window in physical pixels.
    pub fn outer_position(&self) -> (i32, i32) {
        match self.window.outer_position() {
            Ok(pos) => (pos.x, pos.y),
            Err(_) => (0, 0),
        }
    }

    /// Return the current scale factor of the window's monitor.
    #[allow(dead_code)]
    pub fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    /// Check whether the given physical-pixel position falls within any connected monitor.
    pub fn is_position_on_any_monitor(x: i32, y: i32, event_loop: &ActiveEventLoop) -> bool {
        for monitor in event_loop.available_monitors() {
            let pos = monitor.position();
            let size = monitor.size();
            let mx = pos.x;
            let my = pos.y;
            let mw = size.width as i32;
            let mh = size.height as i32;
            if x >= mx && x < mx + mw && y >= my && y < my + mh {
                return true;
            }
        }
        false
    }

    /// Return the centre position of the primary monitor (physical pixels).
    /// Falls back to (0, 0) if no primary monitor is found.
    pub fn primary_monitor_center(event_loop: &ActiveEventLoop) -> (i32, i32) {
        if let Some(monitor) = event_loop.primary_monitor() {
            let pos = monitor.position();
            let size = monitor.size();
            (
                pos.x + size.width as i32 / 2,
                pos.y + size.height as i32 / 2,
            )
        } else {
            // Fallback: try the first available monitor
            if let Some(monitor) = event_loop.available_monitors().next() {
                let pos = monitor.position();
                let size = monitor.size();
                (
                    pos.x + size.width as i32 / 2,
                    pos.y + size.height as i32 / 2,
                )
            } else {
                (0, 0)
            }
        }
    }
}
