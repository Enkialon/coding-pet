// Application state — Phase 8 (Integration + Polish)
// Winit ApplicationHandler that owns the window, renderer, animator, ambient timer, and system tray.
// Drives a frame-accurate animation loop with zero-CPU sleep between frames.
// Loads a real Codex pet if available, otherwise falls back to a test spritesheet.

use std::time::Instant;

use crate::animation::{AmbientTimer, Animator};
use crate::config::Config;
use crate::locale::Locale;
use crate::pet::codex::CodexPetProvider;
use crate::pet::model::{Animation, PetAnimation, SpriteRect};
use crate::pet::{discover_codex_pets, PetProvider};
use crate::renderer::Renderer;
use crate::tray::{check_menu_event, PetTray};
use crate::window::PetWindow;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

/// Default window size for the desktop pet.
const WIN_WIDTH: u32 = 192;
const WIN_HEIGHT: u32 = 208;

/// Number of test animation frames.
const TEST_FRAME_COUNT: u32 = 6;

/// Frame duration for the test animation (milliseconds).
const TEST_FRAME_MS: u64 = 150;

pub struct PetApp {
    window: Option<PetWindow>,
    renderer: Option<Renderer>,
    animator: Animator,
    /// Ambient idle animation timer (triggers random one-shot animations).
    ambient_timer: AmbientTimer,
    /// Optional real pet provider loaded from a Codex pet directory.
    provider: Option<Box<dyn PetProvider>>,
    /// Fallback: premultiplied BGRA pixel data for the multi-frame test spritesheet strip.
    /// Only used when no real pet is available.
    test_spritesheet: Vec<u8>,
    /// Stride (bytes per row) of the fallback test spritesheet.
    test_stride: usize,
    /// System tray manager.
    tray: Option<PetTray>,
    /// Whether the pet window is currently visible.
    visible: bool,
    /// Last known cursor position (physical pixels).
    cursor_x: i32,
    cursor_y: i32,
    /// Whether a mouse drag is in progress.
    dragging: bool,
}

impl PetApp {
    pub fn new(config: &Config) -> Self {
        // Try to discover and load a Codex pet (optionally the one from config)
        let provider = try_load_pet(&config.pet);

        // Always generate the fallback test spritesheet (cheap; used only if provider is None)
        let sheet_width = WIN_WIDTH * TEST_FRAME_COUNT;
        let spritesheet = generate_test_spritesheet(sheet_width, WIN_HEIGHT);
        let stride = sheet_width as usize * 4;

        // Configure ambient timer from config
        let mut ambient_timer = AmbientTimer::new();
        ambient_timer.set_enabled(config.idle_animation);

        PetApp {
            window: None,
            renderer: None,
            animator: Animator::new(),
            ambient_timer,
            provider,
            test_spritesheet: spritesheet,
            test_stride: stride,
            tray: None,
            visible: true,
            cursor_x: 0,
            cursor_y: 0,
            dragging: false,
        }
    }

    /// Return the active spritesheet pixels and stride — from the real provider or the fallback.
    fn active_spritesheet(&self) -> (&[u8], usize) {
        if let Some(provider) = &self.provider {
            (provider.spritesheet_pixels(), provider.spritesheet_stride())
        } else {
            (&self.test_spritesheet, self.test_stride)
        }
    }

    /// Build the initial Idle animation from the provider or the test fallback.
    fn idle_animation(&self) -> Animation {
        if let Some(provider) = &self.provider {
            if let Some(anim) = provider.animation(PetAnimation::Idle) {
                return anim.clone();
            }
        }
        // Fallback: horizontal strip of TEST_FRAME_COUNT frames
        let frames: Vec<SpriteRect> = (0..TEST_FRAME_COUNT)
            .map(|i| SpriteRect {
                x: i * WIN_WIDTH,
                y: 0,
                width: WIN_WIDTH,
                height: WIN_HEIGHT,
            })
            .collect();
        Animation {
            frames,
            frame_duration_ms: TEST_FRAME_MS,
            looping: true,
        }
    }

    /// Try to play the given animation; falls back to Idle if the provider doesn't have it.
    fn play_animation(&mut self, anim: PetAnimation) {
        let animation_data = if let Some(provider) = &self.provider {
            provider
                .animation(anim)
                .cloned()
                .or_else(|| provider.animation(PetAnimation::Idle).cloned())
        } else {
            None
        };

        let animation_data = animation_data.unwrap_or_else(|| self.idle_animation());
        let actual_anim = if self.provider.is_some()
            && self
                .provider
                .as_ref()
                .is_some_and(|p| p.animation(anim).is_some())
        {
            anim
        } else {
            PetAnimation::Idle
        };
        self.animator.set_animation(actual_anim, &animation_data);
        // Reset ambient timer whenever user explicitly triggers an animation
        self.ambient_timer.reset();
        if let Some(win) = &self.window {
            win.window.request_redraw();
        }
    }

    /// Build a list of `(display_name, pet_id)` from discovered pet directories.
    fn discover_pet_names() -> Vec<(String, String)> {
        let pets = discover_codex_pets();
        let mut names = Vec::new();
        for pet_dir in &pets {
            if let Ok(provider) = CodexPetProvider::load(pet_dir) {
                let meta = provider.metadata();
                names.push((meta.display_name.clone(), meta.id.clone()));
            }
        }
        names
    }

    /// Reload a pet by its pet_id (directory name match).
    fn reload_pet_by_id(&mut self, pet_id: &str) {
        let pets = discover_codex_pets();
        for pet_dir in &pets {
            if let Ok(provider) = CodexPetProvider::load(pet_dir) {
                if provider.metadata().id == pet_id {
                    tracing::info!("Switched to pet: {}", provider.metadata().display_name);
                    self.provider = Some(Box::new(provider));
                    self.play_animation(PetAnimation::Idle);
                    return;
                }
            }
        }
        tracing::warn!("Could not find pet with id: {}", pet_id);
    }

    /// Get the pets directory path for "Open Folder" functionality.
    fn pets_folder_path() -> Option<std::path::PathBuf> {
        let home = std::env::var("USERPROFILE")
            .ok()
            .map(std::path::PathBuf::from)?;
        let path = home.join(".codex").join("pets");
        if path.is_dir() {
            Some(path)
        } else {
            None
        }
    }

    /// Save current position and pet selection to config.
    fn save_config(&self) {
        let mut config = Config::load();
        if let Some(win) = &self.window {
            let (x, y) = win.outer_position();
            config.position.x = x;
            config.position.y = y;
        }
        if let Some(provider) = &self.provider {
            config.pet = provider.metadata().id.clone();
        }
        config.always_on_top = self.tray.as_ref().is_some_and(|t| t.always_on_top);
        config.idle_animation = self.ambient_timer.is_enabled();
        config.save();
    }

    /// Handle a menu event from the tray or context menu.
    fn handle_menu_event(&mut self, event: &tray_icon::menu::MenuEvent, event_loop: &ActiveEventLoop) {
        let id = event.id().0.as_ref();

        match id {
            "show_hide" => {
                self.visible = !self.visible;
                if let Some(win) = &self.window {
                    win.window.set_visible(self.visible);
                }
                tracing::info!("Window visibility toggled: {}", self.visible);
            }
            "exit" | "exit_ctx" => {
                tracing::info!("Exit requested via tray menu");
                self.save_config();
                event_loop.exit();
            }
            "anim_idle" => {
                self.play_animation(PetAnimation::Idle);
            }
            "anim_wave" | "wave" => {
                self.play_animation(PetAnimation::Waving);
            }
            "anim_jump" | "jump" => {
                self.play_animation(PetAnimation::Jumping);
            }
            "always_on_top" => {
                if let Some(tray) = &mut self.tray {
                    let new_state = !tray.always_on_top;
                    tray.set_always_on_top_text(new_state);
                    // Toggle WS_EX_TOPMOST via windows-sys
                    if let Some(win) = &self.window {
                        use windows_sys::Win32::UI::WindowsAndMessaging::{
                            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE,
                            HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
                            SWP_NOSIZE, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
                        };
                        unsafe {
                            let ex = GetWindowLongPtrW(win.hwnd as _, GWL_EXSTYLE);
                            let base = (ex as usize & !WS_EX_TOPMOST as usize)
                                | WS_EX_LAYERED as usize
                                | WS_EX_TOOLWINDOW as usize;
                            let new_style = if new_state {
                                base | WS_EX_TOPMOST as usize
                            } else {
                                base
                            };
                            SetWindowLongPtrW(win.hwnd as _, GWL_EXSTYLE, new_style as _);
                            // Must call SetWindowPos to actually change the Z-order
                            let hwnd_insert_after = if new_state {
                                HWND_TOPMOST
                            } else {
                                HWND_NOTOPMOST
                            };
                            SetWindowPos(
                                win.hwnd as _,
                                hwnd_insert_after,
                                0, 0, 0, 0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                        }
                    }
                    tracing::info!("Always on top: {}", new_state);
                }
            }
            "open_folder" | "open_folder_tray" => {
                if let Some(path) = Self::pets_folder_path() {
                    let _ = std::process::Command::new("explorer")
                        .arg(path.as_os_str())
                        .spawn();
                    tracing::info!("Opened pets folder: {}", path.display());
                } else {
                    tracing::warn!("Pets folder not found");
                }
            }
            other => {
                // Check if it's a pet selection item (pet_<id>)
                if let Some(pet_id) = other.strip_prefix("pet_") {
                    self.reload_pet_by_id(pet_id);
                } else {
                    tracing::debug!("Unhandled menu event: {}", other);
                }
            }
        }
    }
}

/// Attempt to discover and load a Codex pet.
/// If `preferred_id` is non-empty, try to load that specific pet first.
fn try_load_pet(preferred_id: &str) -> Option<Box<dyn PetProvider>> {
    let pets = discover_codex_pets();
    if pets.is_empty() {
        tracing::info!("No Codex pets discovered in %USERPROFILE%\\.codex\\pets\\");
        return None;
    }

    tracing::info!("Discovered {} Codex pet(s)", pets.len());

    // If a preferred pet ID is set, try it first
    if !preferred_id.is_empty() {
        for pet_dir in &pets {
            if let Ok(provider) = CodexPetProvider::load(pet_dir) {
                if provider.metadata().id == preferred_id {
                    tracing::info!(
                        "Loaded preferred pet '{}': {}",
                        preferred_id,
                        provider.metadata().display_name
                    );
                    return Some(Box::new(provider));
                }
            }
        }
        tracing::warn!("Preferred pet '{}' not found, falling back to first available", preferred_id);
    }

    // Fall back to first successfully loaded pet
    for pet_dir in &pets {
        match CodexPetProvider::load(pet_dir) {
            Ok(provider) => return Some(Box::new(provider)),
            Err(e) => {
                tracing::warn!(
                    "Failed to load pet from {}: {e}",
                    pet_dir.display()
                );
            }
        }
    }

    tracing::error!("All discovered pets failed to load");
    None
}

impl ApplicationHandler for PetApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return; // already initialised
        }

        // Determine initial window position: config or default centre
        let config = Config::load();
        let (pos_x, pos_y) = if config.position.x != 0 || config.position.y != 0 {
            // Validate saved position is still on a monitor
            if PetWindow::is_position_on_any_monitor(config.position.x, config.position.y, event_loop) {
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

        let win = match PetWindow::new(event_loop, WIN_WIDTH, WIN_HEIGHT, pos_x, pos_y) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        let renderer = match Renderer::new(win.hwnd, WIN_WIDTH, WIN_HEIGHT) {
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

        // Draw the first frame immediately
        let rect = self.animator.current_rect();
        let (pixels, stride) = self.active_spritesheet();
        renderer.draw_frame(pixels, stride, rect);

        // Apply always_on_top from config on startup
        if !config.always_on_top {
            unsafe {
                use windows_sys::Win32::UI::WindowsAndMessaging::*;
                let ex = GetWindowLongPtrW(win.hwnd as _, GWL_EXSTYLE);
                SetWindowLongPtrW(win.hwnd as _, GWL_EXSTYLE, ex & !(WS_EX_TOPMOST as isize));
            }
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
        tracing::info!(
            "Window and renderer initialised — animation started ({source})"
        );
        self.window = Some(win);
        self.renderer = Some(renderer);
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            let now = Instant::now();

            // Tick the animation
            if self.animator.tick(now) {
                // Frame changed — request a redraw
                if let Some(win) = &self.window {
                    win.window.request_redraw();
                }
            }

            // Check if a one-shot animation just finished — return to Idle
            if !self.animator.is_playing() && self.animator.is_finished() {
                let idle_anim = self.idle_animation();
                self.animator.set_animation(PetAnimation::Idle, &idle_anim);
                self.ambient_timer.reset();
                if let Some(win) = &self.window {
                    win.window.request_redraw();
                }
            }

            // Check ambient timer: trigger random one-shot animation when idle
            if self.visible {
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
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                // The tray menu is the single place that explicitly exits or
                // hides the pet. Ignore OS close requests (for example Alt+F4)
                // so the desktop pet and tray icon remain visible.
                tracing::info!("Close requested — keeping desktop pet running");
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = self.renderer.as_ref() {
                    let rect = self.animator.current_rect();
                    let (pixels, stride) = self.active_spritesheet();
                    renderer.draw_frame(pixels, stride, rect);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_x = position.x as i32;
                self.cursor_y = position.y as i32;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                // Initiate window drag (native drag, Aero Snap disabled via subclass)
                if let Some(win) = &self.window {
                    win.drag_window();
                    self.dragging = true;
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging {
                    self.dragging = false;
                    // Save position after drag
                    self.save_config();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                // Show context menu at cursor position
                if let (Some(tray), Some(win)) = (&self.tray, &self.window) {
                    tray.show_context_menu(win.hwnd, self.cursor_x, self.cursor_y);
                }
            }
            _ => {}
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

        // Calculate the earliest wake-up time from animation timer and ambient timer
        let anim_time = if self.animator.is_playing() {
            Some(self.animator.next_frame_time())
        } else {
            None
        };
        let ambient_time = if self.ambient_timer.is_enabled() && self.visible {
            Some(self.ambient_timer.next_trigger_time())
        } else {
            None
        };

        match (anim_time, ambient_time) {
            (Some(a), Some(b)) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(a.min(b)));
            }
            (Some(a), None) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(a));
            }
            (None, Some(b)) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(b));
            }
            (None, None) => {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Test spritesheet generation (fallback)
// ---------------------------------------------------------------------------

/// Generate a horizontal strip of coloured circles — one per frame.
/// Each frame is WIN_WIDTH×WIN_HEIGHT; the total image is (WIN_WIDTH * frame_count) × WIN_HEIGHT.
/// Premultiplied BGRA format.
fn generate_test_spritesheet(total_width: u32, height: u32) -> Vec<u8> {
    let w = total_width as usize;
    let h = height as usize;
    let mut pixels = vec![0u8; w * h * 4];

    // Palette of distinct colours for each frame (R, G, B)
    let palette: [(u8, u8, u8); 6] = [
        (220, 100, 90),  // warm coral
        (90, 180, 220),  // sky blue
        (130, 210, 120), // spring green
        (240, 190, 80),  // golden yellow
        (180, 120, 220), // soft violet
        (240, 140, 180), // rose pink
    ];

    let frame_w = WIN_WIDTH as usize;
    let cx_local = frame_w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let radius = (frame_w.min(h)) as f64 / 2.0 - 4.0;

    for y in 0..h {
        for x in 0..w {
            let frame_idx = x / frame_w;
            let local_x = x % frame_w;

            let dx = local_x as f64 - cx_local;
            let dy = y as f64 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= radius {
                let edge_factor = ((radius - dist).min(1.0)).max(0.0);
                let (r, g, b) = palette[frame_idx % palette.len()];
                let a_f = (200.0 * edge_factor).clamp(0.0, 255.0);
                let a = a_f as u8;

                // Inner highlight towards top-centre
                let hl = ((1.0 - (dy / radius).abs()) * 0.3).clamp(0.0, 1.0);
                let r = ((r as f64 + 60.0 * hl) / 255.0 * a_f) as u8;
                let g = ((g as f64 + 60.0 * hl) / 255.0 * a_f) as u8;
                let b = ((b as f64 + 60.0 * hl) / 255.0 * a_f) as u8;

                let idx = (y * w + x) * 4;
                pixels[idx] = b;
                pixels[idx + 1] = g;
                pixels[idx + 2] = r;
                pixels[idx + 3] = a;
            }
        }
    }

    pixels
}

/// Public helper so main.rs can launch the event loop.
pub fn run() -> Result<(), crate::error::AppError> {
    let config = Config::load();
    let event_loop =
        EventLoop::new().map_err(|e| crate::error::AppError::Window(e.to_string()))?;
    let mut app = PetApp::new(&config);
    event_loop
        .run_app(&mut app)
        .map_err(|e| crate::error::AppError::Window(e.to_string()))?;
    Ok(())
}
