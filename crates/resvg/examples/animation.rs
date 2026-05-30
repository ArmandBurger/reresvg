// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

fn main() {
    let options = usvg::Options::default();
    let data = std::fs::read("crates/resvg/tests/fixtures/spinner.svg")
        .expect("read spinner.svg from the workspace root");

    let animation = usvg::AnimatedSvg::parse(&data, &options).expect("parse animated SVG");
    println!("animated: {}, duration: {}s", animation.is_animated(), animation.duration());

    let sheet = resvg::render_sprite_sheet(
        &animation,
        &options,
        &resvg::FrameOptions { frame_count: 12, ..Default::default() },
        &resvg::SheetOptions { columns: Some(4), padding: 0 },
    )
    .expect("render sprite sheet");

    println!(
        "sheet: {}x{} grid, {}x{} per frame",
        sheet.columns, sheet.rows, sheet.frame_width, sheet.frame_height
    );
    sheet.pixmap.save_png("spinner_sheet.png").expect("save PNG");
    println!("wrote spinner_sheet.png");
}
