//! Assembles the Bevy app: minimal plugins, window, resources, and schedule.

use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin};

use crate::config::DemoConfig;
use crate::systems;

/// Builds and runs the demo application.
pub fn run() {
    let config = DemoConfig::default();
    let timer_seconds = 1.0 / config.fps;

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "resvg animated-icon demo".into(),
                resolution: (760.0, 820.0).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.10, 0.10, 0.12)))
        .insert_resource(config)
        .insert_resource(systems::FrameTimer(Timer::from_seconds(
            timer_seconds,
            TimerMode::Repeating,
        )))
        .insert_resource(systems::RebakeRequested::default())
        .add_systems(Startup, systems::setup)
        .add_systems(
            Update,
            (
                systems::handle_controls,
                systems::rebake_icons,
                systems::advance_frames,
                systems::update_overlay,
                systems::update_background,
            ),
        )
        .run();
}
