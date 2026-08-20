/// Native width/height of one pet animation frame (Codex v2 cell size).
pub const PET_FRAME_WIDTH: u32 = 192;
pub const PET_FRAME_HEIGHT: u32 = 208;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetAnimation {
    Idle,
    RunningRight,
    RunningLeft,
    Waving,
    Jumping,
    Failed,
    Waiting,
    Running,
    Review,
    /// One of the 16 clockwise Codex v2 look poses. Index 0 is up, 4 is
    /// screen-right, 8 is down, and 12 is screen-left.
    Look(u8),
}

#[derive(Debug, Clone, Copy)]
pub struct SpriteRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct Animation {
    pub frames: Vec<SpriteRect>,
    /// Duration for each frame. Codex uses state-specific timing and often a
    /// longer hold on the final frame.
    pub frame_durations_ms: Vec<u64>,
    pub looping: bool,
}

impl Animation {
    pub fn frame_duration_ms(&self, index: usize) -> u64 {
        self.frame_durations_ms
            .get(index)
            .copied()
            .or_else(|| self.frame_durations_ms.last().copied())
            .unwrap_or(150)
    }
}

#[derive(Debug, Clone)]
pub struct PetMetadata {
    pub id: String,
    pub display_name: String,
    #[allow(dead_code)]
    pub description: Option<String>,
}

/// Future event hook placeholder — NOT used in V0.1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PetEvent {
    Idle,
    Working,
    Completed,
    NeedsInput,
    Failed,
}
