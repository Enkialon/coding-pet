// Animation engine — Phase 3
// State machine that drives sprite animation with precise frame timing.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::pet::model::{Animation, PetAnimation, SpriteRect};

pub struct Animator {
    current: PetAnimation,
    frame_index: usize,
    frame_count: usize,
    frames: Vec<SpriteRect>,
    frame_durations: Vec<Duration>,
    looping: bool,
    last_advance: Instant,
    playing: bool,
    finished: bool,
}

impl Animator {
    /// Create a new animator starting in Idle state (not playing).
    pub fn new() -> Self {
        Animator {
            current: PetAnimation::Idle,
            frame_index: 0,
            frame_count: 0,
            frames: Vec::new(),
            frame_durations: vec![Duration::from_millis(150)],
            looping: true,
            last_advance: Instant::now(),
            playing: false,
            finished: false,
        }
    }

    /// Switch to a new animation, resetting all frame state and starting playback.
    pub fn set_animation(&mut self, anim: PetAnimation, animation_data: &Animation) {
        self.current = anim;
        self.frame_index = 0;
        self.frame_count = animation_data.frames.len();
        self.frames = animation_data.frames.clone();
        self.frame_durations = (0..animation_data.frames.len())
            .map(|index| Duration::from_millis(animation_data.frame_duration_ms(index)))
            .collect();
        self.looping = animation_data.looping;
        self.playing = true;
        self.finished = false;
        self.last_advance = Instant::now();
    }

    /// Advance the animation if enough time has elapsed.
    /// Returns `true` when the visible frame actually changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        if !self.playing || self.frame_count == 0 {
            return false;
        }

        if now.duration_since(self.last_advance) < self.current_frame_duration() {
            return false;
        }

        self.last_advance = now;

        let next = self.frame_index + 1;
        if next < self.frame_count {
            self.frame_index = next;
            true
        } else if self.looping {
            self.frame_index = 0;
            true
        } else {
            // Non-looping animation: stay on last frame, stop playing
            self.playing = false;
            self.finished = true;
            false
        }
    }

    /// The sprite rect for the current frame.
    pub fn current_rect(&self) -> SpriteRect {
        self.frames[self.frame_index]
    }

    /// When the next frame should be displayed.
    pub fn next_frame_time(&self) -> Instant {
        self.last_advance + self.current_frame_duration()
    }

    /// Whether the animator is actively playing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// The currently active animation variant.
    pub fn current_animation(&self) -> PetAnimation {
        self.current
    }

    /// Returns true if a non-looping animation has reached its last frame.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Freeze on the current frame (stop advancing).
    pub fn pause(&mut self) {
        self.playing = false;
    }

    fn current_frame_duration(&self) -> Duration {
        self.frame_durations
            .get(self.frame_index)
            .copied()
            .or_else(|| self.frame_durations.last().copied())
            .unwrap_or_else(|| Duration::from_millis(150))
    }
}

/// Simple ambient animation timer that triggers random animations
/// when the pet has been idle for a while.
pub struct AmbientTimer {
    next_trigger: Instant,
    enabled: bool,
}

impl AmbientTimer {
    pub fn new() -> Self {
        Self {
            next_trigger: Instant::now() + Self::random_interval(),
            enabled: true,
        }
    }

    /// Check if it's time to trigger an ambient animation.
    /// Returns Some(animation) if triggered, None otherwise.
    pub fn check(&mut self, now: Instant) -> Option<PetAnimation> {
        if !self.enabled {
            return None;
        }
        if now >= self.next_trigger {
            self.next_trigger = now + Self::random_interval();
            Some(Self::random_animation(now))
        } else {
            None
        }
    }

    /// Reset the timer (call when animation starts from user interaction)
    pub fn reset(&mut self) {
        self.next_trigger = Instant::now() + Self::random_interval();
    }

    /// Get the next trigger time (for WaitUntil scheduling)
    pub fn next_trigger_time(&self) -> Instant {
        self.next_trigger
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Random interval between 20-60 seconds
    fn random_interval() -> Duration {
        // Simple randomness using system time nanos — no rand crate needed
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(42))
            .subsec_nanos();
        let seconds = 20 + (nanos % 41); // 20..=60
        Duration::from_secs(seconds as u64)
    }

    /// Pick a random ambient animation (Waving or Jumping)
    fn random_animation(_now: Instant) -> PetAnimation {
        // Use elapsed nanos as simple randomness source
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .subsec_nanos();
        if nanos % 2 == 0 {
            PetAnimation::Waving
        } else {
            PetAnimation::Jumping
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::model::{Animation, PetAnimation, SpriteRect};

    fn test_animation(frames: usize, duration_ms: u64, looping: bool) -> Animation {
        Animation {
            frames: (0..frames)
                .map(|i| SpriteRect {
                    x: i as u32 * 192,
                    y: 0,
                    width: 192,
                    height: 208,
                })
                .collect(),
            frame_durations_ms: vec![duration_ms; frames],
            looping,
        }
    }

    #[test]
    fn test_frame_advance() {
        let anim = test_animation(4, 100, true);
        let mut animator = Animator::new();
        animator.set_animation(PetAnimation::Idle, &anim);

        let start = Instant::now();
        // Should be at frame 0
        assert_eq!(animator.current_rect().x, 0);

        // Advance past one frame duration
        let changed = animator.tick(start + Duration::from_millis(101));
        assert!(changed);
        assert_eq!(animator.current_rect().x, 192);
    }

    #[test]
    fn test_loop_wrapping() {
        let anim = test_animation(3, 100, true);
        let mut animator = Animator::new();
        animator.set_animation(PetAnimation::Idle, &anim);

        let start = Instant::now();
        // Advance past all frames
        animator.tick(start + Duration::from_millis(101));
        animator.tick(start + Duration::from_millis(201));
        let changed = animator.tick(start + Duration::from_millis(301));
        assert!(changed);
        // Should wrap to frame 0
        assert_eq!(animator.current_rect().x, 0);
    }

    #[test]
    fn test_non_looping_stops() {
        let anim = test_animation(3, 100, false);
        let mut animator = Animator::new();
        animator.set_animation(PetAnimation::Waving, &anim);

        let start = Instant::now();
        animator.tick(start + Duration::from_millis(101));
        animator.tick(start + Duration::from_millis(201));
        animator.tick(start + Duration::from_millis(301)); // past last frame

        assert!(animator.is_finished());
        // Should stay at last frame (index 2)
        assert_eq!(animator.current_rect().x, 384);
    }

    #[test]
    fn test_uses_per_frame_durations() {
        let mut anim = test_animation(2, 100, true);
        anim.frame_durations_ms = vec![100, 300];
        let mut animator = Animator::new();
        animator.set_animation(PetAnimation::Idle, &anim);

        let start = Instant::now();
        assert!(animator.tick(start + Duration::from_millis(101)));
        assert_eq!(animator.current_rect().x, 192);
        assert!(!animator.tick(start + Duration::from_millis(250)));
        assert!(animator.tick(start + Duration::from_millis(402)));
        assert_eq!(animator.current_rect().x, 0);
    }

    #[test]
    fn test_paused_pose_is_not_finished() {
        let anim = test_animation(1, 100, false);
        let mut animator = Animator::new();
        animator.set_animation(PetAnimation::Look(8), &anim);
        animator.pause();

        assert!(!animator.is_playing());
        assert!(!animator.is_finished());
    }

    #[test]
    fn selecting_the_same_animation_restarts_from_first_frame() {
        let animation = test_animation(3, 10, true);
        let mut animator = Animator::new();
        animator.set_animation(PetAnimation::Running, &animation);
        assert!(animator.tick(Instant::now() + Duration::from_millis(20)));
        assert_eq!(animator.current_rect().x, 192);

        animator.set_animation(PetAnimation::Running, &animation);
        assert_eq!(animator.current_rect().x, 0);
        assert!(animator.is_playing());
    }
}
