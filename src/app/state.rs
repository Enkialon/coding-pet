use std::time::Instant;

use crate::animation::{AmbientTimer, Animator};
use crate::pet::model::PetAnimation;
use crate::pet::PetProvider;
use crate::platform::{PetWindow, Renderer};
use crate::ui::tray::PetTray;

pub(super) struct PetApp {
    pub(super) window: Option<PetWindow>,
    pub(super) renderer: Option<Renderer>,
    pub(super) animator: Animator,
    pub(super) ambient_timer: AmbientTimer,
    pub(super) provider: Option<Box<dyn PetProvider>>,
    pub(super) test_spritesheet: Vec<u8>,
    pub(super) test_stride: usize,
    pub(super) tray: Option<PetTray>,
    pub(super) visible: bool,
    pub(super) cursor_x: i32,
    pub(super) cursor_y: i32,
    pub(super) cursor_inside: bool,
    pub(super) dragging: bool,
    pub(super) looking: bool,
    pub(super) current_look_direction: Option<u8>,
    pub(super) last_look_update: Option<Instant>,
    pub(super) look_update_pending: bool,
    pub(super) cursor_leave_deadline: Option<Instant>,
    pub(super) last_drag_window_x: Option<i32>,
    pub(super) base_animation: PetAnimation,
    pub(super) pending_animation: Option<PetAnimation>,
}
