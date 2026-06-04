// iai-callgrind instruction-count benchmarks for the usvg parser. Deterministic
// and machine-independent: this is the CI regression gate. Same generators as
// the Criterion harness, smaller sizes (Valgrind is ~50x slower).

use std::hint::black_box;

use iai_callgrind::{library_benchmark, library_benchmark_group, main};

#[path = "generators.rs"]
mod generators;

use generators::{
    css_heavy, flat_rects, gradient_heavy, nested_groups, path_segments, single_kind,
};

fn parse(svg: String) -> usvg::Tree {
    let options = usvg::Options::default();
    usvg::Tree::from_str(black_box(&svg), &options).expect("generated SVG must parse")
}

#[library_benchmark]
#[bench::flat(flat_rects(1_000))]
#[bench::nested(nested_groups(1_000))]
#[bench::gradient(gradient_heavy(1_000))]
#[bench::css(css_heavy(1_000))]
#[bench::path(path_segments(1_000))]
fn scaled(svg: String) -> usvg::Tree {
    parse(svg)
}

#[library_benchmark]
#[bench::rect(single_kind("rect", 200))]
#[bench::circle(single_kind("circle", 200))]
#[bench::ellipse(single_kind("ellipse", 200))]
#[bench::line(single_kind("line", 200))]
#[bench::polyline(single_kind("polyline", 200))]
#[bench::polygon(single_kind("polygon", 200))]
#[bench::path(single_kind("path", 200))]
#[bench::group(single_kind("g", 200))]
#[bench::use_ref(single_kind("use", 200))]
#[bench::symbol(single_kind("symbol", 200))]
#[bench::image(single_kind("image", 200))]
#[bench::linear_gradient(single_kind("linearGradient", 200))]
#[bench::radial_gradient(single_kind("radialGradient", 200))]
#[bench::pattern(single_kind("pattern", 200))]
#[bench::clip_path(single_kind("clipPath", 200))]
#[bench::mask(single_kind("mask", 200))]
#[bench::filter(single_kind("filter", 200))]
#[bench::marker(single_kind("marker", 200))]
#[bench::animate(single_kind("animate", 200))]
#[bench::animate_transform(single_kind("animateTransform", 200))]
#[bench::set(single_kind("set", 200))]
#[bench::animate_motion(single_kind("animateMotion", 200))]
fn per_element_kind(svg: String) -> usvg::Tree {
    parse(svg)
}

library_benchmark_group!(name = parsing; benchmarks = scaled, per_element_kind);
main!(library_benchmark_groups = parsing);
