//! Platform boundary for native desktop integration.
//!
//! Animation, pet assets, configuration, and hook handling stay platform-neutral.
//! Each desktop backend owns its window creation and frame presentation details.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::{
    configure_event_loop, global_cursor_position, open_folder, NativeWindowHandle, PetWindow,
    Renderer,
};
#[cfg(target_os = "windows")]
pub use windows::{
    configure_event_loop, global_cursor_position, open_folder, NativeWindowHandle, PetWindow,
    Renderer,
};

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
compile_error!("CodingPet currently supports Windows and macOS");
