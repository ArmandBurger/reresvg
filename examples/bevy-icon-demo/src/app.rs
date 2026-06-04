//! Assembles the Bevy app: minimal plugins, window, resources, and schedule.

use bevy::log::{Level, LogPlugin};
use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin};

use crate::config::DemoConfig;
use crate::systems;

/// Builds and runs the demo application.
pub fn run() {
    let config = DemoConfig::default();
    let timer_seconds = 1.0 / config.fps;

    let window_plugin = WindowPlugin {
        primary_window: Some(Window {
            title: "reresvg animated-icon demo".into(),
            resolution: (760.0, 820.0).into(),
            ..default()
        }),
        ..default()
    };

    // Keep the wgpu/naga backend chatter down while surfacing this crate's own
    // diagnostics (e.g. a failed icon bake) at debug level.
    let logging_plugin = LogPlugin {
        level: Level::INFO,
        filter: "wgpu=error,naga=warn,bevy_icon_demo=debug".into(),
        ..default()
    };

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(window_plugin)
                .set(logging_plugin),
        )
        .insert_resource(ClearColor(Color::srgb(0.10, 0.10, 0.12)))
        .insert_resource(config)
        .insert_resource(systems::FrameTimer(Timer::from_seconds(
            timer_seconds,
            TimerMode::Repeating,
        )))
        .insert_resource(systems::RebakeRequested::default())
        .insert_resource(systems::StepRequested::default())
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
