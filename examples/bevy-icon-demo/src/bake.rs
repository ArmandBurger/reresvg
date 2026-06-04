//! Bakes an animated SVG into a sprite sheet and converts the pixels into the
//! straight-alpha RGBA layout Bevy's sprite pipeline expects.

use reresvg::tiny_skia::IntSize;
use reresvg::usvg::{AnimatedSvg, Options};
use reresvg::{FrameOptions, SheetOptions};

/// A baked sprite sheet in Bevy-ready straight-alpha RGBA8, plus its grid metadata.
pub struct BakedSheet {
    /// Straight-alpha RGBA8 bytes, row-major, `sheet_width * sheet_height * 4` long.
    pub rgba: Vec<u8>,
    pub sheet_width: u32,
    pub sheet_height: u32,
    pub frame_width: u32,
    pub frame_height: u32,
    pub columns: u32,
    pub rows: u32,
    pub frame_count: usize,
}

/// Why a bake failed.
#[derive(Debug)]
pub enum BakeError {
    /// The SVG parsed but contains no supported animation.
    NotAnimated,
    /// The requested render size was invalid (zero).
    BadSize,
    /// The underlying usvg/resvg pipeline failed.
    Render(reresvg::usvg::Error),
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakeError::NotAnimated => write!(formatter, "SVG contains no supported animation"),
            BakeError::BadSize => write!(formatter, "invalid render size"),
            BakeError::Render(error) => write!(formatter, "render failed: {error}"),
        }
    }
}

impl std::error::Error for BakeError {}

impl From<reresvg::usvg::Error> for BakeError {
    fn from(error: reresvg::usvg::Error) -> BakeError {
        BakeError::Render(error)
    }
}

/// Bakes one animated SVG into a straight-alpha sprite sheet.
///
/// `frame_count` frames are sampled across the animation's own timeline, each
/// rendered at `size`×`size` pixels, packed into a near-square grid with
/// `padding` pixels between cells.
pub fn bake_icon(svg: &[u8], frame_count: usize, size: u32, padding: u32) -> Result<BakedSheet, BakeError> {
    let options = Options::default();
    let animation = AnimatedSvg::parse(svg, &options)?;
    if !animation.is_animated() {
        return Err(BakeError::NotAnimated);
    }

    let frame_size = IntSize::from_wh(size, size).ok_or(BakeError::BadSize)?;
    let frame_options = FrameOptions {
        frame_count,
        size: Some(frame_size),
        ..Default::default()
    };
    let sheet_options = SheetOptions { columns: None, padding };

    let sheet = reresvg::render_sprite_sheet(&animation, &options, &frame_options, &sheet_options)?;
    let rgba = demultiply_to_straight(sheet.pixmap.data());

    Ok(BakedSheet {
        rgba,
        sheet_width: sheet.pixmap.width(),
        sheet_height: sheet.pixmap.height(),
        frame_width: sheet.frame_width,
        frame_height: sheet.frame_height,
        columns: sheet.columns,
        rows: sheet.rows,
        frame_count: sheet.frame_count,
    })
}

/// Converts premultiplied RGBA8 bytes (as produced by tiny-skia) into straight
/// (non-premultiplied) RGBA8 bytes. Fully transparent pixels become all-zero.
pub fn demultiply_to_straight(premultiplied: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(premultiplied.len());
    for pixel in premultiplied.chunks_exact(4) {
        let alpha = pixel[3];
        if alpha == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let unpremultiply = |channel: u8| -> u8 {
            (((channel as u16 * 255) + (alpha as u16) / 2) / alpha as u16).min(255) as u8
        };
        out.extend_from_slice(&[
            unpremultiply(pixel[0]),
            unpremultiply(pixel[1]),
            unpremultiply(pixel[2]),
            alpha,
        ]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::demultiply_to_straight;
    use super::{bake_icon, BakeError};
    use crate::icons::ICONS;

    #[test]
    fn bakes_spinner_into_a_grid() {
        let spinner = ICONS.iter().find(|icon| icon.name == "spinner").unwrap();
        let baked = bake_icon(spinner.svg, 12, 64, 0).unwrap();
        assert_eq!(baked.frame_count, 12);
        assert_eq!(baked.frame_width, 64);
        assert_eq!(baked.frame_height, 64);
        assert_eq!(baked.columns * baked.frame_width, baked.sheet_width);
        assert_eq!(baked.rgba.len() as u32, baked.sheet_width * baked.sheet_height * 4);
        assert!(baked.rgba.iter().any(|byte| *byte != 0), "sheet is fully blank");
    }

    #[test]
    fn static_svg_reports_not_animated() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="4" height="4"/></svg>"#;
        assert!(matches!(bake_icon(svg, 4, 32, 0), Err(BakeError::NotAnimated)));
    }

    #[test]
    fn opaque_pixel_is_unchanged() {
        let input = [10, 20, 30, 255];
        assert_eq!(demultiply_to_straight(&input), vec![10, 20, 30, 255]);
    }

    #[test]
    fn transparent_pixel_becomes_zero() {
        let input = [128, 64, 64, 0];
        assert_eq!(demultiply_to_straight(&input), vec![0, 0, 0, 0]);
    }

    #[test]
    fn half_alpha_red_recovers_full_red() {
        // Premultiplied 50%-alpha red: r = 255*128/255 ≈ 128, a = 128.
        let input = [128, 0, 0, 128];
        let output = demultiply_to_straight(&input);
        assert_eq!(output[3], 128);
        assert!(output[0] >= 254, "expected ~255 red, got {}", output[0]);
        assert_eq!(&output[1..3], &[0, 0]);
    }
}
