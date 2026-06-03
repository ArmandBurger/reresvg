//! Bakes an animated SVG into a sprite sheet and converts the pixels into the
//! straight-alpha RGBA layout Bevy's sprite pipeline expects.

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
