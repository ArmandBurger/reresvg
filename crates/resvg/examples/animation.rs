// Copyright 2026 the Resvg Authors
// Copyright 2026 Armand Burger
// SPDX-License-Identifier: MIT

fn main() {
    let options = usvg::Options::default();
    let data = std::fs::read("crates/resvg/tests/fixtures/spinner.svg")
        .expect("read spinner.svg from the workspace root");

    let animation = usvg::AnimatedSvg::parse(&data, &options).expect("parse animated SVG");
    println!("animated: {}, duration: {}s", animation.is_animated(), animation.duration());

    let sheet = reresvg::render_sprite_sheet(
        &animation,
        &options,
        &reresvg::FrameOptions { frame_count: 12, ..Default::default() },
        &reresvg::SheetOptions { columns: Some(4), padding: 0 },
    )
    .expect("render sprite sheet");

    println!(
        "sheet: {}x{} grid, {}x{} per frame",
        sheet.columns, sheet.rows, sheet.frame_width, sheet.frame_height
    );
    sheet.pixmap.save_png("spinner_sheet.png").expect("save PNG");
    println!("wrote spinner_sheet.png");
}
