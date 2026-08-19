// Application state — Phase 8 (Integration + Polish)
// Winit ApplicationHandler that owns the window, renderer, animator, ambient timer, and system tray.
// Drives a frame-accurate animation loop with zero-CPU sleep between frames.
// Loads a real Codex pet if available, otherwise falls back to a test spritesheet.

use std::time::{Duration, Instant};

use crate::animation::{AmbientTimer, Animator};
use crate::config::Config;
use crate::integration::{AgentEventKind, AppUserEvent, IntegrationServer};
use crate::locale::Locale;
use crate::pet::codex::CodexPetProvider;
use crate::pet::model::{Animation, PetAnimation, SpriteRect};
use crate::pet::{codex_pets_dir, discover_codex_pets, PetProvider};
use crate::platform::{self, PetWindow, Renderer};
use crate::tray::{check_menu_event, PetTray};
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
    /// Whether the pointer is currently inside the pet window.
    cursor_inside: bool,
    /// Whether a mouse drag is in progress.
    dragging: bool,
    /// Whether a Codex v2 look pose currently overrides the agent state.
    looking: bool,
    /// Current stabilized v2 direction index.
    current_look_direction: Option<u8>,
    /// Last time a pending raw cursor position was applied to the sprite.
    last_look_update: Option<Instant>,
    /// Whether the latest raw cursor position still needs to be applied.
    look_update_pending: bool,
    /// Delayed confirmation for transient CursorLeft events.
    cursor_leave_deadline: Option<Instant>,
    /// Last window x-coordinate observed during a native drag.
    last_drag_window_x: Option<i32>,
    /// Persistent animation restored after pointer interaction ends.
    base_animation: PetAnimation,
    /// Latest hook animation deferred while a pointer interaction has priority.
    pending_animation: Option<PetAnimation>,
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

/// Convert a pointer position into a stabilized Codex v2 clockwise direction.
/// Index 0 is up, 4 right, 8 down, and 12 left.
fn stabilized_look_direction_index(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    current: Option<u8>,
) -> Option<u8> {
    let dx = f64::from(x) - f64::from(width) / 2.0;
    let dy = f64::from(y) - f64::from(height) / 2.0;
    let radius_squared = dx * dx + dy * dy;
    let threshold = if current.is_some() {
        LOOK_EXIT_RADIUS
    } else {
        LOOK_ENTER_RADIUS
    };
    if radius_squared <= threshold * threshold {
        return None;
    }

    let mut angle = dx.atan2(-dy);
    if angle < 0.0 {
        angle += std::f64::consts::TAU;
    }
    Some(quantize_look_angle(angle, current))
}

fn normalize_cursor_position(
    x: f64,
    y: f64,
    surface_width: u32,
    surface_height: u32,
) -> (i32, i32) {
    (
        (x * f64::from(WIN_WIDTH) / f64::from(surface_width.max(1))) as i32,
        (y * f64::from(WIN_HEIGHT) / f64::from(surface_height.max(1))) as i32,
    )
}

fn quantize_look_angle(angle: f64, current: Option<u8>) -> u8 {
    let step = std::f64::consts::TAU / 16.0;
    if let Some(current) = current {
        let center = f64::from(current) * step;
        let distance = (angle - center + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        if distance.abs() <= step / 2.0 + LOOK_ANGLE_HYSTERESIS {
            return current;
        }
    }
    ((angle / step).round() as u8) % 16
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

        let win = match PetWindow::new(event_loop, WIN_WIDTH, WIN_HEIGHT, pos_x, pos_y) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        let mut renderer = match Renderer::new(&win, WIN_WIDTH, WIN_HEIGHT) {
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

            self.confirm_cursor_leave(now);
            self.flush_look_update(now);

            // Tick the animation
            if self.animator.tick(now) {
                // Frame changed — request a redraw
                if let Some(win) = &self.window {
                    win.window.request_redraw();
                }
            }

            // Check if a one-shot animation just finished — freeze in idle pose
            if !self.animator.is_playing() && self.animator.is_finished() {
                self.freeze_idle();
            }

            // Check ambient timer: trigger random one-shot animation when idle
            if self.visible && !self.dragging && !self.looking {
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
                self.queue_look_update(Instant::now(), false);
            }
            WindowEvent::CursorEntered { .. } => {
                self.cursor_inside = true;
                self.cursor_leave_deadline = None;
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor_inside = false;
                self.look_update_pending = false;
                if self.looking && !self.dragging {
                    self.cursor_leave_deadline = Some(Instant::now() + CURSOR_LEAVE_DELAY);
                }
            }
            WindowEvent::Moved(position) => {
                if self.dragging {
                    if let Some(previous_x) = self.last_drag_window_x.replace(position.x) {
                        let delta_x = position.x - previous_x;
                        if delta_x > 0 {
                            self.play_drag_animation(PetAnimation::RunningRight);
                        } else if delta_x < 0 {
                            self.play_drag_animation(PetAnimation::RunningLeft);
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                // Initiate window drag (native drag, Aero Snap disabled via subclass)
                self.dragging = true;
                self.looking = false;
                self.current_look_direction = None;
                self.look_update_pending = false;
                self.cursor_leave_deadline = None;
                self.last_drag_window_x = self.window.as_ref().map(|win| win.outer_position().0);
                let initial_animation = if self.cursor_x >= WIN_WIDTH as i32 / 2 {
                    PetAnimation::RunningRight
                } else {
                    PetAnimation::RunningLeft
                };
                self.play_drag_animation(initial_animation);
                let drag_started = self.window.as_ref().is_some_and(|win| win.drag_window());
                if !drag_started {
                    self.dragging = false;
                    self.last_drag_window_x = None;
                    self.restore_base_animation();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging {
                    self.dragging = false;
                    self.last_drag_window_x = None;
                    // Save position after drag
                    self.save_config();
                    if self.cursor_inside {
                        self.queue_look_update(Instant::now(), true);
                    }
                    if !self.looking {
                        self.restore_base_animation();
                    }
                }
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
                let interacting = self.dragging || self.looking;
                let animation = match event.kind() {
                    // When the agent is idle, freeze the pet in its idle pose
                    // instead of playing a looping idle animation.
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
                        self.freeze_idle();
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
        let ambient_time =
            if self.ambient_timer.is_enabled() && self.visible && !self.dragging && !self.looking {
                Some(self.ambient_timer.next_trigger_time())
            } else {
                None
            };
        let look_time = if self.look_update_pending && self.cursor_inside && !self.dragging {
            Some(
                self.last_look_update
                    .map(|last| last + LOOK_THROTTLE)
                    .unwrap_or_else(Instant::now),
            )
        } else {
            None
        };

        let next_wake = [
            anim_time,
            ambient_time,
            look_time,
            self.cursor_leave_deadline,
        ]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_direction_cardinals_follow_codex_clockwise_order() {
        assert_eq!(
            stabilized_look_direction_index(96, 0, 192, 208, None),
            Some(0)
        );
        assert_eq!(
            stabilized_look_direction_index(191, 104, 192, 208, None),
            Some(4)
        );
        assert_eq!(
            stabilized_look_direction_index(96, 207, 192, 208, None),
            Some(8)
        );
        assert_eq!(
            stabilized_look_direction_index(0, 104, 192, 208, None),
            Some(12)
        );
    }

    #[test]
    fn look_direction_diagonal_and_deadzone() {
        assert_eq!(
            stabilized_look_direction_index(191, 0, 192, 208, None),
            Some(2)
        );
        assert_eq!(
            stabilized_look_direction_index(96, 104, 192, 208, None),
            None
        );
        assert_eq!(
            stabilized_look_direction_index(105, 110, 192, 208, None),
            None
        );
    }

    #[test]
    fn look_deadzone_uses_enter_exit_hysteresis() {
        // Twenty pixels right of center is too close to enter Look.
        assert_eq!(
            stabilized_look_direction_index(116, 104, 192, 208, None),
            None
        );
        // Once looking, the same point remains in the current direction.
        assert_eq!(
            stabilized_look_direction_index(116, 104, 192, 208, Some(4)),
            Some(4)
        );
        // Moving inside the smaller exit radius returns to the base state.
        assert_eq!(
            stabilized_look_direction_index(106, 104, 192, 208, Some(4)),
            None
        );
    }

    #[test]
    fn look_angle_hysteresis_prevents_sector_chatter() {
        let thirteen_degrees = 13.0_f64.to_radians();
        let eighteen_degrees = 18.0_f64.to_radians();

        // The normal boundary is 11.25°, but an active direction is retained
        // until the cursor clears the additional 6° hysteresis margin.
        assert_eq!(quantize_look_angle(thirteen_degrees, Some(0)), 0);
        assert_eq!(quantize_look_angle(eighteen_degrees, Some(0)), 1);
    }

    #[test]
    fn cursor_position_is_normalized_on_retina_surfaces() {
        assert_eq!(normalize_cursor_position(192.0, 208.0, 384, 416), (96, 104));
        assert_eq!(normalize_cursor_position(96.0, 104.0, 192, 208), (96, 104));
    }
}
