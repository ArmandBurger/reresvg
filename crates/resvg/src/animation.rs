// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use usvg::AnimatedSvg;

/// A sampling window over the animation timeline, in seconds.
#[derive(Clone, Copy, Debug)]
pub struct TimeSpan {
    /// Start of the sampling window in seconds.
    pub start: f64,
    /// End of the sampling window in seconds.
    pub end: f64,
}

/// Controls how an animation is sampled into frames.
pub struct FrameOptions {
    /// Number of frames to render (e.g. 12 chunky, 60 smooth).
    pub frame_count: usize,
    /// Sampling window; `None` means `0..duration`.
    pub time_span: Option<TimeSpan>,
    /// Output size per frame; `None` means the tree's own size.
    pub size: Option<tiny_skia::IntSize>,
    /// `false` (default) omits the loop endpoint so frame N does not duplicate frame 0.
    pub endpoint_inclusive: bool,
    /// Root transform applied to every frame.
    pub transform: tiny_skia::Transform,
}

impl Default for FrameOptions {
    fn default() -> FrameOptions {
        FrameOptions {
            frame_count: 1,
            time_span: None,
            size: None,
            endpoint_inclusive: false,
            transform: tiny_skia::Transform::identity(),
        }
    }
}

/// Renders `frame_count` frames sampled across the animation timeline.
///
/// Frames are sampled endpoint-exclusively by default (suitable for seamless loops).
/// Each frame is an independent [`tiny_skia::Pixmap`] allocated at the requested size
/// or at the SVG's natural size when `frame_options.size` is `None`.
///
/// Returns `Ok(Vec::new())` when `frame_options.frame_count == 0`.
pub fn render_frames(
    animation: &AnimatedSvg,
    options: &usvg::Options,
    frame_options: &FrameOptions,
) -> Result<Vec<tiny_skia::Pixmap>, usvg::Error> {
    if frame_options.frame_count == 0 {
        return Ok(Vec::new());
    }

    let span = frame_options.time_span.unwrap_or(TimeSpan {
        start: 0.0,
        end: animation.duration(),
    });

    let times = sample_times(span, frame_options.frame_count, frame_options.endpoint_inclusive);
    let mut frames = Vec::with_capacity(times.len());

    for time in times {
        let tree = animation.tree_at(time, options)?;
        let size = frame_options
            .size
            .unwrap_or_else(|| tree.size().to_int_size());
        let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
            .ok_or(usvg::Error::InvalidSize)?;

        let scale_x = size.width() as f32 / tree.size().width();
        let scale_y = size.height() as f32 / tree.size().height();
        let transform = frame_options.transform.pre_scale(scale_x, scale_y);

        crate::render(&tree, transform, &mut pixmap.as_mut());
        frames.push(pixmap);
    }

    Ok(frames)
}

/// A packed sprite sheet: a single pixmap arranged as a uniform grid of animation frames.
pub struct SpriteSheet {
    /// The composited pixmap containing all frames in a grid layout.
    pub pixmap: tiny_skia::Pixmap,
    /// Number of columns in the grid.
    pub columns: u32,
    /// Number of rows in the grid.
    pub rows: u32,
    /// Width of each individual frame cell in pixels.
    pub frame_width: u32,
    /// Height of each individual frame cell in pixels.
    pub frame_height: u32,
    /// Total number of frames packed into the sheet (may be less than `columns * rows`).
    pub frame_count: usize,
}

/// Controls sprite-sheet grid layout.
#[derive(Default)]
pub struct SheetOptions {
    /// Number of columns in the grid; `None` uses `ceil(sqrt(frame_count))` for a near-square layout.
    pub columns: Option<u32>,
    /// Inter-cell spacing in pixels added between frames (but not on the outer edges).
    pub padding: u32,
}

/// Packs a slice of uniform-sized frames into a single grid pixmap.
///
/// Returns `None` if `frames` is empty. All frames are assumed to be the same
/// size; the first frame's dimensions are used for the cell size. Any remaining
/// cells in the last row are left transparent.
pub fn pack_sprite_sheet(
    frames: &[tiny_skia::Pixmap],
    sheet_options: &SheetOptions,
) -> Option<SpriteSheet> {
    let first = frames.first()?;
    let frame_width = first.width();
    let frame_height = first.height();
    let frame_count = frames.len();

    let columns = sheet_options
        .columns
        .unwrap_or_else(|| (frame_count as f64).sqrt().ceil() as u32)
        .max(1);
    let rows = (frame_count as u32).div_ceil(columns);

    let padding = sheet_options.padding;
    let sheet_width = columns * frame_width + padding * columns.saturating_sub(1);
    let sheet_height = rows * frame_height + padding * rows.saturating_sub(1);

    let mut pixmap = tiny_skia::Pixmap::new(sheet_width, sheet_height)?;
    let paint = tiny_skia::PixmapPaint::default();

    for (index, frame) in frames.iter().enumerate() {
        let column = (index as u32) % columns;
        let row = (index as u32) / columns;
        let x = (column * (frame_width + padding)) as i32;
        let y = (row * (frame_height + padding)) as i32;
        pixmap.draw_pixmap(
            x,
            y,
            frame.as_ref(),
            &paint,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    Some(SpriteSheet {
        pixmap,
        columns,
        rows,
        frame_width,
        frame_height,
        frame_count,
    })
}

/// Renders an animated SVG directly into a packed sprite sheet.
///
/// This is a convenience that calls [`render_frames`] followed by [`pack_sprite_sheet`],
/// mapping an empty frame list (which can only happen when `frame_options.frame_count == 0`)
/// to [`usvg::Error::InvalidSize`].
pub fn render_sprite_sheet(
    animation: &AnimatedSvg,
    options: &usvg::Options,
    frame_options: &FrameOptions,
    sheet_options: &SheetOptions,
) -> Result<SpriteSheet, usvg::Error> {
    let frames = render_frames(animation, options, frame_options)?;
    pack_sprite_sheet(&frames, sheet_options).ok_or(usvg::Error::InvalidSize)
}

fn sample_times(span: TimeSpan, frame_count: usize, endpoint_inclusive: bool) -> Vec<f64> {
    let length = span.end - span.start;
    if frame_count == 1 {
        return vec![span.start];
    }
    (0..frame_count)
        .map(|index| {
            let divisor = if endpoint_inclusive {
                (frame_count - 1) as f64
            } else {
                frame_count as f64
            };
            span.start + length * (index as f64) / divisor
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPINNER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20">
        <g>
            <rect x="8" y="2" width="4" height="4" fill="#ff0000"/>
            <animateTransform attributeName="transform" type="rotate" from="0 10 10" to="360 10 10" dur="1s"/>
        </g>
    </svg>"##;

    #[test]
    fn renders_requested_frame_count() {
        let options = usvg::Options::default();
        let animation = usvg::AnimatedSvg::parse(SPINNER.as_bytes(), &options).unwrap();
        let frames = render_frames(&animation, &options, &FrameOptions { frame_count: 12, ..Default::default() }).unwrap();
        assert_eq!(frames.len(), 12);
        assert_eq!(frames[0].width(), 20);
        assert_eq!(frames[0].height(), 20);
    }

    #[test]
    fn frames_differ_across_rotation() {
        let options = usvg::Options::default();
        let animation = usvg::AnimatedSvg::parse(SPINNER.as_bytes(), &options).unwrap();
        let frames = render_frames(&animation, &options, &FrameOptions { frame_count: 4, ..Default::default() }).unwrap();
        assert_ne!(frames[0].data(), frames[1].data());
    }

    #[test]
    fn zero_frames_is_empty() {
        let options = usvg::Options::default();
        let animation = usvg::AnimatedSvg::parse(SPINNER.as_bytes(), &options).unwrap();
        let frames = render_frames(&animation, &options, &FrameOptions { frame_count: 0, ..Default::default() }).unwrap();
        assert!(frames.is_empty());
    }

    #[test]
    fn packs_uniform_grid() {
        let mut frames = Vec::new();
        for _ in 0..7 {
            frames.push(tiny_skia::Pixmap::new(10, 8).unwrap());
        }
        let sheet = pack_sprite_sheet(&frames, &SheetOptions { columns: Some(3), padding: 0 }).unwrap();
        assert_eq!(sheet.columns, 3);
        assert_eq!(sheet.rows, 3);
        assert_eq!(sheet.frame_width, 10);
        assert_eq!(sheet.frame_height, 8);
        assert_eq!(sheet.pixmap.width(), 30);
        assert_eq!(sheet.pixmap.height(), 24);
        assert_eq!(sheet.frame_count, 7);
    }

    #[test]
    fn default_columns_are_near_square() {
        let frames: Vec<_> = (0..9).map(|_| tiny_skia::Pixmap::new(4, 4).unwrap()).collect();
        let sheet = pack_sprite_sheet(&frames, &SheetOptions { columns: None, padding: 0 }).unwrap();
        assert_eq!(sheet.columns, 3);
        assert_eq!(sheet.rows, 3);
    }

    #[test]
    fn empty_frames_yield_none() {
        assert!(pack_sprite_sheet(&[], &SheetOptions { columns: None, padding: 0 }).is_none());
    }

    #[test]
    fn renders_fixture_sprite_sheet() {
        let options = usvg::Options::default();
        let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/spinner.svg")).unwrap();
        let animation = usvg::AnimatedSvg::parse(&data, &options).unwrap();
        assert!(animation.is_animated());
        let sheet = render_sprite_sheet(
            &animation,
            &options,
            &FrameOptions { frame_count: 12, ..Default::default() },
            &SheetOptions { columns: Some(4), padding: 0 },
        )
        .unwrap();
        assert_eq!(sheet.pixmap.width(), 256);
        assert_eq!(sheet.pixmap.height(), 192);
        assert!(sheet.pixmap.data().iter().any(|byte| *byte != 0));
    }
}
