use std::time::Instant;

use crate::animation::{AmbientTimer, Animator};
use crate::pet::model::PetAnimation;
use crate::pet::PetProvider;
use crate::platform::{PetWindow, Renderer};
use crate::ui::settings::SettingsWindow;
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
    pub(super) pointer: PointerGesture,
    pub(super) looking: bool,
    pub(super) current_look_direction: Option<u8>,
    pub(super) next_pointer_poll: Instant,
    pub(super) pointer_look_suppressed_until: Option<Instant>,
    pub(super) base_animation: PetAnimation,
    pub(super) pending_animation: Option<PetAnimation>,
    /// Whether idle should loop. Owned here so a future settings page can
    /// toggle it without waiting for the next config reload.
    pub(super) loop_idle: bool,
    /// Auto-allow Codex `PermissionRequest` approvals via the hook adapter.
    pub(super) auto_approve_permissions: bool,
    /// Bind the integration API for LAN forward hooks. Applied on next launch.
    pub(super) integration_listen_lan: bool,
    pub(super) look_range: i32,
    pub(super) display_scale: f64,
    /// How often to sample the global cursor for the awareness ring.
    pub(super) pointer_poll_ms: u64,
    pub(super) settings: Option<SettingsWindow>,
    /// Snapshot of startup-only config fields taken in `PetApp::new`, so
    /// `resumed()` and the settings window never re-read the config file.
    pub(super) initial_position: (i32, i32),
    pub(super) initial_always_on_top: bool,
    pub(super) config_language: String,
}

/// Left-button gesture parsed from the raw winit pointer stream.
/// Press is not a drag; drag starts only after the cursor moves far enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PointerGesture {
    Idle,
    Pressed {
        cursor: (i32, i32),
        offset: (i32, i32),
        window_x: i32,
    },
    Dragging {
        last_window_x: i32,
        offset: (i32, i32),
    },
}

impl PointerGesture {
    pub(super) fn is_dragging(self) -> bool {
        matches!(self, Self::Dragging { .. })
    }

    /// Pressed or dragging: the button is down, so Look/ambient should wait.
    pub(super) fn is_held(self) -> bool {
        !matches!(self, Self::Idle)
    }

    pub(super) fn set_last_window_x(&mut self, x: i32) {
        if let Self::Dragging { last_window_x, .. } = self {
            *last_window_x = x;
        }
    }
}
