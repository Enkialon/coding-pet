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
    pub frame_duration_ms: u64,
    pub looping: bool,
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
