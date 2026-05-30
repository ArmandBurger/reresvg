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
}
