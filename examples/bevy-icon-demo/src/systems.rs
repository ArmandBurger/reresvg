//! Bevy glue: turns baked sheets into textures/atlases, spawns the grid, and
//! runs playback, controls, re-baking, overlay, and background systems.

use bevy::prelude::*;
use bevy::image::Image;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::bake::{bake_icon, BakedSheet};
use crate::config::{next_frame_index, Background, DemoConfig};
use crate::grid::cell_translation;
use crate::icons::ICONS;

/// On-screen display size of each icon cell's sprite, in pixels.
const SPRITE_DISPLAY_SIZE: f32 = 150.0;
/// Spacing between grid cells, in pixels.
const CELL_SIZE: f32 = 220.0;
/// Magenta placeholder shown when an icon fails to bake.
const PLACEHOLDER_COLOR: Color = Color::srgb(1.0, 0.0, 1.0);

/// Marks a sprite that plays a baked icon; carries its grid index and ping-pong state.
#[derive(Component)]
pub struct IconCell {
    pub index: usize,
    pub frame_count: usize,
    pub descending: bool,
}

/// Marks the on-screen configuration overlay text.
#[derive(Component)]
pub struct OverlayText;

/// Marks the tiled checkerboard background sprite.
#[derive(Component)]
pub struct CheckerBackground;

/// Drives playback at the configured FPS.
#[derive(Resource)]
pub struct FrameTimer(pub Timer);

/// Set when a config change requires re-baking all icons.
#[derive(Resource, Default)]
pub struct RebakeRequested(pub bool);

/// Set for one frame when the user requests a single-step while paused.
#[derive(Resource, Default)]
pub struct StepRequested(pub bool);

/// Builds a Bevy `Image` (straight-alpha sRGB) from a baked sheet.
pub fn baked_to_image(baked: &BakedSheet) -> Image {
    Image::new(
        Extent3d {
            width: baked.sheet_width,
            height: baked.sheet_height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        baked.rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

/// Builds a 2×2 checkerboard `Image` for the tiled background.
fn checker_image() -> Image {
    let light: [u8; 4] = [80, 80, 80, 255];
    let dark: [u8; 4] = [50, 50, 50, 255];
    let mut data = Vec::with_capacity(16);
    data.extend_from_slice(&light);
    data.extend_from_slice(&dark);
    data.extend_from_slice(&dark);
    data.extend_from_slice(&light);
    Image::new(
        Extent3d { width: 2, height: 2, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

/// Spawns the camera, the checkerboard background, the 3×3 grid of icon sprites
/// with labels, and the overlay text. Failed bakes become magenta placeholders.
pub fn setup(
    mut commands: Commands,
    config: Res<DemoConfig>,
    mut images: ResMut<Assets<Image>>,
    mut atlases: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.spawn(Camera2d);

    // Tiled checkerboard background, hidden unless Background::Checker is active.
    let checker_handle = images.add(checker_image());
    let mut checker_sprite = Sprite::from_image(checker_handle);
    checker_sprite.custom_size = Some(Vec2::new(2000.0, 2000.0));
    checker_sprite.image_mode = SpriteImageMode::Tiled {
        tile_x: true,
        tile_y: true,
        stretch_value: 0.02,
    };
    commands.spawn((
        checker_sprite,
        Transform::from_xyz(0.0, 0.0, -10.0),
        Visibility::Hidden,
        CheckerBackground,
    ));

    for (index, icon) in ICONS.iter().enumerate() {
        let (x, y) = cell_translation(index, CELL_SIZE);

        match bake_icon(icon.svg, config.frame_count, config.render_size, config.padding) {
            Ok(baked) => {
                let layout = TextureAtlasLayout::from_grid(
                    UVec2::new(baked.frame_width, baked.frame_height),
                    baked.columns,
                    baked.rows,
                    if config.padding > 0 { Some(UVec2::splat(config.padding)) } else { None },
                    None,
                );
                let layout_handle = atlases.add(layout);
                let image_handle = images.add(baked_to_image(&baked));

                let mut sprite = Sprite::from_atlas_image(
                    image_handle,
                    TextureAtlas { layout: layout_handle, index: 0 },
                );
                sprite.custom_size = Some(Vec2::splat(SPRITE_DISPLAY_SIZE));

                commands.spawn((
                    sprite,
                    Transform::from_xyz(x, y, 0.0),
                    IconCell { index, frame_count: baked.frame_count, descending: false },
                ));
            }
            Err(error) => {
                bevy::log::error!("icon {} failed to bake: {error}", icon.name);
                commands.spawn((
                    Sprite {
                        color: PLACEHOLDER_COLOR,
                        custom_size: Some(Vec2::splat(SPRITE_DISPLAY_SIZE)),
                        ..default()
                    },
                    Transform::from_xyz(x, y, 0.0),
                    IconCell { index, frame_count: 1, descending: false },
                ));
            }
        }

        // Label beneath the cell.
        commands.spawn((
            Text2d::new(icon.label),
            TextFont { font_size: 14.0, ..default() },
            TextColor(Color::WHITE),
            Transform::from_xyz(x, y - SPRITE_DISPLAY_SIZE / 2.0 - 16.0, 1.0),
        ));
    }

    // Configuration overlay (top-left).
    commands.spawn((
        Text::new(""),
        TextFont { font_size: 16.0, ..default() },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
        OverlayText,
    ));
}

/// Advances every icon sprite's atlas index according to the timer and loop mode.
pub fn advance_frames(
    time: Res<Time>,
    config: Res<DemoConfig>,
    mut timer: ResMut<FrameTimer>,
    mut step: ResMut<StepRequested>,
    mut query: Query<(&mut Sprite, &mut IconCell)>,
) {
    let stepping = step.0;
    step.0 = false;

    let advance = if config.paused {
        stepping
    } else {
        timer.0.tick(time.delta()).just_finished()
    };
    if !advance {
        return;
    }

    for (mut sprite, mut cell) in query.iter_mut() {
        let Some(atlas) = sprite.texture_atlas.as_mut() else {
            continue;
        };
        let (next, descending) =
            next_frame_index(atlas.index, cell.descending, cell.frame_count, config.loop_mode);
        atlas.index = next;
        cell.descending = descending;
    }
}

/// Translates key presses into config changes, flagging a re-bake when a change
/// affects the baked sheets (frame count, render size, padding, reset).
pub fn handle_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<DemoConfig>,
    mut timer: ResMut<FrameTimer>,
    mut rebake: ResMut<RebakeRequested>,
    mut step: ResMut<StepRequested>,
    mut exit: EventWriter<AppExit>,
) {
    let mut timer_dirty = false;

    if keys.just_pressed(KeyCode::Space) {
        config.paused = !config.paused;
    }
    if keys.just_pressed(KeyCode::Period) && config.paused {
        // Request a single-step; advance_frames consumes this flag next frame.
        step.0 = true;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        config.change_fps(1.0);
        timer_dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        config.change_fps(-1.0);
        timer_dirty = true;
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        config.change_frame_count(1);
        rebake.0 = true;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        config.change_frame_count(-1);
        rebake.0 = true;
    }
    if keys.just_pressed(KeyCode::Equal) {
        config.change_size(16);
        rebake.0 = true;
    }
    if keys.just_pressed(KeyCode::Minus) {
        config.change_size(-16);
        rebake.0 = true;
    }
    if keys.just_pressed(KeyCode::Comma) {
        config.cycle_padding();
        rebake.0 = true;
    }
    if keys.just_pressed(KeyCode::KeyL) {
        config.loop_mode = config.loop_mode.next();
    }
    if keys.just_pressed(KeyCode::KeyB) {
        config.background = config.background.next();
    }
    if keys.just_pressed(KeyCode::KeyR) {
        *config = DemoConfig::default();
        rebake.0 = true;
        timer_dirty = true;
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }

    if timer_dirty {
        let seconds = 1.0 / config.fps;
        timer.0.set_duration(std::time::Duration::from_secs_f32(seconds));
    }
}

/// When a re-bake is requested, re-bakes every icon at the current config and
/// swaps each cell's texture and atlas layout in place. Keeps the old texture
/// on failure.
pub fn rebake_icons(
    config: Res<DemoConfig>,
    mut rebake: ResMut<RebakeRequested>,
    mut images: ResMut<Assets<Image>>,
    mut atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut query: Query<(&mut Sprite, &mut IconCell)>,
) {
    if !rebake.0 {
        return;
    }
    rebake.0 = false;

    for (mut sprite, mut cell) in query.iter_mut() {
        let icon = &ICONS[cell.index];
        match bake_icon(icon.svg, config.frame_count, config.render_size, config.padding) {
            Ok(baked) => {
                let layout = TextureAtlasLayout::from_grid(
                    UVec2::new(baked.frame_width, baked.frame_height),
                    baked.columns,
                    baked.rows,
                    if config.padding > 0 { Some(UVec2::splat(config.padding)) } else { None },
                    None,
                );
                sprite.image = images.add(baked_to_image(&baked));
                if let Some(atlas) = sprite.texture_atlas.as_mut() {
                    atlas.layout = atlases.add(layout);
                    atlas.index = 0;
                }
                cell.frame_count = baked.frame_count;
                cell.descending = false;
            }
            Err(error) => {
                bevy::log::error!("re-bake of {} failed, keeping previous: {error}", icon.name);
            }
        }
    }
}

/// Refreshes the on-screen config readout every frame.
pub fn update_overlay(config: Res<DemoConfig>, mut query: Query<&mut Text, With<OverlayText>>) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let loop_mode = match config.loop_mode {
        crate::config::LoopMode::Loop => "loop",
        crate::config::LoopMode::PingPong => "ping-pong",
        crate::config::LoopMode::Once => "once",
    };
    let background = match config.background {
        Background::Dark => "dark",
        Background::Light => "light",
        Background::Checker => "checker",
    };
    text.0 = format!(
        "frames [ ]: {}   size - =: {}px   pad ,: {}px   fps Up/Dn: {:.0}\n\
         loop L: {}   bg B: {}   {}   space=pause  .=step  R=reset  Esc=quit",
        config.frame_count,
        config.render_size,
        config.padding,
        config.fps,
        loop_mode,
        background,
        if config.paused { "PAUSED" } else { "playing" },
    );
}

/// Applies the background mode: clear color for dark/light, and toggles the
/// tiled checkerboard sprite's visibility for checker.
pub fn update_background(
    config: Res<DemoConfig>,
    mut clear: ResMut<ClearColor>,
    mut query: Query<&mut Visibility, With<CheckerBackground>>,
) {
    clear.0 = match config.background {
        Background::Dark => Color::srgb(0.10, 0.10, 0.12),
        Background::Light => Color::srgb(0.86, 0.86, 0.88),
        Background::Checker => Color::srgb(0.10, 0.10, 0.12),
    };
    if let Ok(mut visibility) = query.single_mut() {
        *visibility = if config.background == Background::Checker {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
