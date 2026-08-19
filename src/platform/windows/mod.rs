mod renderer;
mod window;

pub use renderer::Renderer;
pub use window::PetWindow;

use std::path::Path;
use winit::event_loop::EventLoopBuilder;

#[derive(Clone, Copy)]
pub enum NativeWindowHandle {
    Win32(isize),
}

pub fn configure_event_loop<T: 'static>(_builder: &mut EventLoopBuilder<T>) {}

pub fn open_folder(path: &Path) -> std::io::Result<()> {
    std::process::Command::new("explorer")
        .arg(path.as_os_str())
        .spawn()
        .map(|_| ())
}

pub fn global_cursor_position(_scale_factor: f64) -> Option<(i32, i32)> {
    let mut point = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    let succeeded =
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point) };
    (succeeded != 0).then_some((point.x, point.y))
}
