mod renderer;
mod window;

pub use renderer::Renderer;
pub use window::PetWindow;

use std::path::Path;
use winit::event_loop::EventLoopBuilder;
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

#[derive(Clone, Copy)]
pub enum NativeWindowHandle {
    AppKit(*const std::ffi::c_void),
}

/// Run as a menu-bar accessory so CodingPet has no Dock icon.
pub fn configure_event_loop<T: 'static>(builder: &mut EventLoopBuilder<T>) {
    builder
        .with_activation_policy(ActivationPolicy::Accessory)
        .with_default_menu(false)
        .with_activate_ignoring_other_apps(false);
}

pub fn open_folder(path: &Path) -> std::io::Result<()> {
    std::process::Command::new("open")
        .arg(path.as_os_str())
        .spawn()
        .map(|_| ())
}

pub fn global_cursor_position(scale_factor: f64) -> Option<(i32, i32)> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
    let point = CGEvent::new(source).ok()?.location();
    Some((
        (point.x * scale_factor).round() as i32,
        (point.y * scale_factor).round() as i32,
    ))
}
