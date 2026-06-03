//! Configuration model for the demo: the live-tunable settings, their
//! clamping/cycling rules, and the frame-index advance used during playback.
//! The logic is Bevy-free so it is fully unit-testable; `DemoConfig` only
//! additionally derives `Resource` so the app can store it.

use bevy::ecs::resource::Resource;

/// How playback loops once the last frame is reached.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoopMode {
    Loop,
    PingPong,
    Once,
}

impl LoopMode {
    /// Cycles Loop -> PingPong -> Once -> Loop.
    pub fn next(self) -> LoopMode {
        match self {
            LoopMode::Loop => LoopMode::PingPong,
            LoopMode::PingPong => LoopMode::Once,
            LoopMode::Once => LoopMode::Loop,
        }
    }
}

/// Background style behind the icons.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Background {
    Dark,
    Light,
    Checker,
}

impl Background {
    /// Cycles Dark -> Light -> Checker -> Dark.
    pub fn next(self) -> Background {
        match self {
            Background::Dark => Background::Light,
            Background::Light => Background::Checker,
            Background::Checker => Background::Dark,
        }
    }
}

/// Inclusive clamp bounds for the tunable numeric settings.
pub const FRAME_COUNT_MIN: usize = 2;
pub const FRAME_COUNT_MAX: usize = 48;
pub const SIZE_MIN: u32 = 32;
pub const SIZE_MAX: u32 = 256;
pub const FPS_MIN: f32 = 1.0;
pub const FPS_MAX: f32 = 60.0;
pub const PADDING_STEPS: [u32; 3] = [0, 2, 8];

/// Live, user-tunable demo configuration.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct DemoConfig {
    pub frame_count: usize,
    pub render_size: u32,
    pub padding: u32,
    pub fps: f32,
    pub loop_mode: LoopMode,
    pub background: Background,
    pub paused: bool,
}

impl Default for DemoConfig {
    fn default() -> DemoConfig {
        DemoConfig {
            frame_count: 12,
            render_size: 96,
            padding: 0,
            fps: 12.0,
            loop_mode: LoopMode::Loop,
            background: Background::Dark,
            paused: false,
        }
    }
}

impl DemoConfig {
    pub fn change_frame_count(&mut self, delta: i32) {
        let next = (self.frame_count as i32 + delta).clamp(FRAME_COUNT_MIN as i32, FRAME_COUNT_MAX as i32);
        self.frame_count = next as usize;
    }

    pub fn change_size(&mut self, delta: i32) {
        let next = (self.render_size as i32 + delta).clamp(SIZE_MIN as i32, SIZE_MAX as i32);
        self.render_size = next as u32;
    }

    pub fn change_fps(&mut self, delta: f32) {
        self.fps = (self.fps + delta).clamp(FPS_MIN, FPS_MAX);
    }

    /// Advances padding to the next value in `PADDING_STEPS` (wrapping).
    pub fn cycle_padding(&mut self) {
        let current = PADDING_STEPS.iter().position(|value| *value == self.padding).unwrap_or(0);
        self.padding = PADDING_STEPS[(current + 1) % PADDING_STEPS.len()];
    }
}

/// Advances a frame index for one playback tick, returning the next
/// `(index, ping_pong_descending)` pair. `descending` is only meaningful for
/// `LoopMode::PingPong` and should be threaded back in on the next call.
pub fn next_frame_index(
    index: usize,
    descending: bool,
    frame_count: usize,
    loop_mode: LoopMode,
) -> (usize, bool) {
    if frame_count <= 1 {
        return (0, false);
    }
    let last = frame_count - 1;
    match loop_mode {
        LoopMode::Loop => ((index + 1) % frame_count, false),
        LoopMode::Once => (index.saturating_add(1).min(last), false),
        LoopMode::PingPong => {
            if descending {
                if index == 0 {
                    (1.min(last), false)
                } else {
                    (index - 1, true)
                }
            } else if index >= last {
                (last.saturating_sub(1), true)
            } else {
                (index + 1, false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let config = DemoConfig::default();
        assert_eq!(config.frame_count, 12);
        assert_eq!(config.render_size, 96);
        assert_eq!(config.fps, 12.0);
        assert_eq!(config.loop_mode, LoopMode::Loop);
        assert!(!config.paused);
    }

    #[test]
    fn frame_count_clamps() {
        let mut config = DemoConfig::default();
        config.change_frame_count(-100);
        assert_eq!(config.frame_count, FRAME_COUNT_MIN);
        config.change_frame_count(1000);
        assert_eq!(config.frame_count, FRAME_COUNT_MAX);
    }

    #[test]
    fn padding_cycles_through_steps() {
        let mut config = DemoConfig::default();
        assert_eq!(config.padding, 0);
        config.cycle_padding();
        assert_eq!(config.padding, 2);
        config.cycle_padding();
        assert_eq!(config.padding, 8);
        config.cycle_padding();
        assert_eq!(config.padding, 0);
    }

    #[test]
    fn loop_mode_wraps_forever() {
        let mut index = 0;
        for _ in 0..5 {
            (index, _) = next_frame_index(index, false, 3, LoopMode::Loop);
        }
        // 0->1->2->0->1->2
        assert_eq!(index, 2);
    }

    #[test]
    fn once_holds_on_last_frame() {
        let (index, _) = next_frame_index(2, false, 3, LoopMode::Once);
        assert_eq!(index, 2);
    }

    #[test]
    fn ping_pong_bounces_at_the_ends() {
        // Ascending hits the top and turns around.
        let (index, descending) = next_frame_index(2, false, 3, LoopMode::PingPong);
        assert_eq!((index, descending), (1, true));
        // Descending hits the bottom and turns around.
        let (index, descending) = next_frame_index(0, true, 3, LoopMode::PingPong);
        assert_eq!((index, descending), (1, false));
    }
}
