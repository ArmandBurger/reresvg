use bevy_icon_demo::bake::bake_icon;
use bevy_icon_demo::icons::ICONS;

#[test]
fn every_icon_bakes_into_a_valid_animated_sheet() {
    for icon in ICONS.iter() {
        let baked = bake_icon(icon.svg, 12, 64, 0)
            .unwrap_or_else(|error| panic!("icon {} failed to bake: {error}", icon.name));

        assert_eq!(baked.frame_count, 12, "icon {} frame_count", icon.name);
        assert_eq!(baked.frame_width, 64, "icon {} frame_width", icon.name);
        assert_eq!(baked.frame_height, 64, "icon {} frame_height", icon.name);
        assert_eq!(
            baked.rgba.len() as u32,
            baked.sheet_width * baked.sheet_height * 4,
            "icon {} buffer length",
            icon.name
        );
        assert!(
            baked.rgba.iter().any(|byte| *byte != 0),
            "icon {} rendered a fully blank sheet",
            icon.name
        );
    }
}
