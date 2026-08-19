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
