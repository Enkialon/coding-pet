// Application lifecycle and orchestration.
// Winit ApplicationHandler that owns the window, renderer, animator, ambient timer, and system tray.
// Drives a frame-accurate animation loop with zero-CPU sleep between frames.
// Loads a real Codex pet if available, otherwise falls back to a test spritesheet.

use std::time::{Duration, Instant};

use crate::animation::{AmbientTimer, Animator};
use crate::config::Config;
use crate::integration::{AppUserEvent, IntegrationServer};
use crate::pet::codex::CodexPetProvider;
use crate::pet::model::{Animation, PetAnimation, SpriteRect};
use crate::pet::{codex_pets_dir, discover_codex_pets, PetProvider};
use crate::platform;
use winit::event_loop::{ActiveEventLoop, EventLoop};

mod interaction;
mod state;

use interaction::{normalize_cursor_position, stabilized_look_direction_index};
use state::PetApp;

/// Default window size for the desktop pet.
const WIN_WIDTH: u32 = 192;
const WIN_HEIGHT: u32 = 208;

/// Number of test animation frames.
const TEST_FRAME_COUNT: u32 = 6;

/// Frame duration for the test animation (milliseconds).
const TEST_FRAME_MS: u64 = 150;

/// Pointer tracking stays responsive without redrawing for every raw move.
const LOOK_THROTTLE: Duration = Duration::from_millis(50);
/// A larger enter radius and smaller exit radius form a Schmitt-trigger-style
/// deadzone so the pet does not oscillate between Idle and Look.
const LOOK_ENTER_RADIUS: f64 = 26.0;
const LOOK_EXIT_RADIUS: f64 = 16.0;
/// Keep the current direction slightly beyond the normal sector boundary.
const LOOK_ANGLE_HYSTERESIS: f64 = 6.0_f64.to_radians();
/// Layered windows can briefly report CursorLeft while their sprite changes.
const CURSOR_LEAVE_DELAY: Duration = Duration::from_millis(80);

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
            cursor_inside: false,
            dragging: false,
            looking: false,
            current_look_direction: None,
            last_look_update: None,
            look_update_pending: false,
            cursor_leave_deadline: None,
            last_drag_window_x: None,
            base_animation: PetAnimation::Idle,
            pending_animation: None,
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
            frame_durations_ms: vec![TEST_FRAME_MS; TEST_FRAME_COUNT as usize],
            looping: true,
        }
    }

    /// Freeze the pet on the first frame of its Idle animation.
    /// Used when the agent reports idle or a one-shot animation finishes,
    /// so the pet stays completely still until the next event.
    fn freeze_idle(&mut self) {
        let idle_anim = self.idle_animation();
        self.animator.set_animation(PetAnimation::Idle, &idle_anim);
        self.animator.pause();
        self.ambient_timer.reset();
        if let Some(win) = &self.window {
            win.window.request_redraw();
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

    /// Display one static Codex v2 pose without treating it as a completed
    /// one-shot animation.
    fn show_pose(&mut self, anim: PetAnimation) -> bool {
        let Some(animation) = self
            .provider
            .as_ref()
            .and_then(|provider| provider.animation(anim))
            .cloned()
        else {
            return false;
        };

        self.animator.set_animation(anim, &animation);
        self.animator.pause();
        if let Some(win) = &self.window {
            win.window.request_redraw();
        }
        true
    }

    fn restore_base_animation(&mut self) {
        self.looking = false;
        self.current_look_direction = None;
        self.look_update_pending = false;
        if let Some(animation) = self.pending_animation.take() {
            self.play_animation(animation);
            return;
        }
        if self.base_animation == PetAnimation::Idle {
            self.freeze_idle();
        } else {
            self.play_animation(self.base_animation);
        }
    }

    fn update_look_pose(&mut self) {
        if self.dragging || !self.cursor_inside {
            return;
        }

        match stabilized_look_direction_index(
            self.cursor_x,
            self.cursor_y,
            WIN_WIDTH,
            WIN_HEIGHT,
            self.current_look_direction,
        ) {
            Some(index) => {
                let animation = PetAnimation::Look(index);
                if self.current_look_direction == Some(index) && self.looking {
                    return;
                }
                if self.show_pose(animation) {
                    self.looking = true;
                    self.current_look_direction = Some(index);
                } else {
                    self.looking = false;
                    self.current_look_direction = None;
                }
            }
            None if self.looking => self.restore_base_animation(),
            None => {}
        }
    }

    fn queue_look_update(&mut self, now: Instant, immediate: bool) {
        self.look_update_pending = true;
        if immediate {
            self.last_look_update = None;
        }
        self.flush_look_update(now);
    }

    fn flush_look_update(&mut self, now: Instant) {
        if !self.look_update_pending || self.dragging || !self.cursor_inside {
            return;
        }
        if self
            .last_look_update
            .is_some_and(|last| now < last + LOOK_THROTTLE)
        {
            return;
        }

        self.look_update_pending = false;
        self.last_look_update = Some(now);
        self.update_look_pose();
    }

    fn confirm_cursor_leave(&mut self, now: Instant) {
        let Some(deadline) = self.cursor_leave_deadline else {
            return;
        };
        if now < deadline {
            return;
        }

        self.cursor_leave_deadline = None;
        if !self.cursor_inside && self.looking && !self.dragging {
            self.restore_base_animation();
        }
    }

    fn play_drag_animation(&mut self, animation: PetAnimation) {
        if self.animator.current_animation() != animation {
            self.play_animation(animation);
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
        let path = codex_pets_dir()?;
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
    fn handle_menu_event(
        &mut self,
        event: &tray_icon::menu::MenuEvent,
        event_loop: &ActiveEventLoop,
    ) {
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
                    if let Some(win) = &self.window {
                        win.set_always_on_top(new_state);
                    }
                    tracing::info!("Always on top: {}", new_state);
                }
            }
            "open_folder" | "open_folder_tray" => {
                if let Some(path) = Self::pets_folder_path() {
                    if let Err(error) = platform::open_folder(&path) {
                        tracing::warn!("Could not open pets folder: {error}");
                    }
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
        tracing::info!("No Codex pets discovered in ~/.codex/pets/");
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
        tracing::warn!(
            "Preferred pet '{}' not found, falling back to first available",
            preferred_id
        );
    }

    // Fall back to first successfully loaded pet
    for pet_dir in &pets {
        match CodexPetProvider::load(pet_dir) {
            Ok(provider) => return Some(Box::new(provider)),
            Err(e) => {
                tracing::warn!("Failed to load pet from {}: {e}", pet_dir.display());
            }
        }
    }

    tracing::error!("All discovered pets failed to load");
    None
}

mod events;

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
    let mut builder = EventLoop::<AppUserEvent>::with_user_event();
    platform::configure_event_loop(&mut builder);
    let event_loop = builder
        .build()
        .map_err(|e| crate::error::AppError::Window(e.to_string()))?;
    let _integration_server = match IntegrationServer::start(event_loop.create_proxy()) {
        Ok(server) => Some(server),
        Err(error) => {
            tracing::warn!("Integration API could not start: {error}");
            None
        }
    };
    let mut app = PetApp::new(&config);
    event_loop
        .run_app(&mut app)
        .map_err(|e| crate::error::AppError::Window(e.to_string()))?;
    Ok(())
}
