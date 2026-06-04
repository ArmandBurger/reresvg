//! The fixed set of demo icons, embedded into the binary so it is
//! self-contained and the tests use the exact same bytes.

/// One animated demo icon: a stable id, a human-facing label, and its SVG bytes.
pub struct IconSource {
    pub name: &'static str,
    pub label: &'static str,
    pub svg: &'static [u8],
}

/// All nine demo icons, in 3×3 grid order (row-major).
pub const ICONS: [IconSource; 9] = [
    IconSource { name: "spinner", label: "Spinner (rotate)", svg: include_bytes!("../assets/icons/spinner.svg") },
    IconSource { name: "bouncing-dot", label: "Bounce (cy/paced)", svg: include_bytes!("../assets/icons/bouncing-dot.svg") },
    IconSource { name: "pulse", label: "Pulse (scale)", svg: include_bytes!("../assets/icons/pulse.svg") },
    IconSource { name: "slider", label: "Slide (translate)", svg: include_bytes!("../assets/icons/slider.svg") },
    IconSource { name: "fade", label: "Fade (opacity)", svg: include_bytes!("../assets/icons/fade.svg") },
    IconSource { name: "color-cycle", label: "Color (fill lerp)", svg: include_bytes!("../assets/icons/color-cycle.svg") },
    IconSource { name: "blink", label: "Blink (discrete)", svg: include_bytes!("../assets/icons/blink.svg") },
    IconSource { name: "comet", label: "Comet (motion)", svg: include_bytes!("../assets/icons/comet.svg") },
    IconSource { name: "combined", label: "Combined (multi)", svg: include_bytes!("../assets/icons/combined.svg") },
];

#[cfg(test)]
mod tests {
    use super::ICONS;

    #[test]
    fn there_are_nine_icons_with_nonempty_bytes() {
        assert_eq!(ICONS.len(), 9);
        for icon in ICONS.iter() {
            assert!(!icon.name.is_empty(), "icon has empty name");
            assert!(!icon.label.is_empty(), "icon {} has empty label", icon.name);
            assert!(!icon.svg.is_empty(), "icon {} has empty svg bytes", icon.name);
        }
    }
}
