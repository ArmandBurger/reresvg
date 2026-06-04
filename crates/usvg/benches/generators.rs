// Deterministic synthetic SVG generators for parser benchmarks. No randomness:
// every value derives from the element index so inputs are byte-stable across
// runs. Inputs contain no <text>, keeping font I/O out of the measurement.

#![allow(dead_code)] // each bench/test target uses a different subset.

use std::fmt::Write;

const HEADER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 1000 1000">"#;

/// `n` sibling rects with index-derived geometry. Stresses element throughput
/// and attribute parsing.
pub fn flat_rects(n: usize) -> String {
    let mut svg = String::from(HEADER);
    for i in 0..n {
        let x = (i % 100) as f64 * 2.0;
        let y = (i / 100) as f64 * 2.0;
        let _ = write!(svg, r##"<rect x="{x}" y="{y}" width="1.5" height="1.5" fill="#3366cc"/>"##);
    }
    svg.push_str("</svg>");
    svg
}

/// `n`-deep nested groups, each with a transform. Stresses tree depth and
/// transform inheritance.
pub fn nested_groups(n: usize) -> String {
    let mut svg = String::from(HEADER);
    for i in 0..n {
        let _ = write!(svg, r#"<g transform="translate(0.1 0.1) rotate({})">"#, i % 360);
    }
    svg.push_str(r#"<rect width="4" height="4"/>"#);
    for _ in 0..n {
        svg.push_str("</g>");
    }
    svg.push_str("</svg>");
    svg
}

/// `n` gradients in <defs>, each actually applied as a fill so the converter
/// keeps them. Stresses paint-server resolution and defs lookup.
pub fn gradient_heavy(n: usize) -> String {
    let mut svg = String::from(HEADER);
    svg.push_str("<defs>");
    for i in 0..n {
        let _ = write!(
            svg,
            r##"<linearGradient id="g{i}"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient>"##
        );
    }
    svg.push_str("</defs>");
    for i in 0..n {
        let x = (i % 100) as f64 * 2.0;
        let y = (i / 100) as f64 * 2.0;
        let _ = write!(svg, r##"<rect x="{x}" y="{y}" width="1.5" height="1.5" fill="url(#g{i})"/>"##);
    }
    svg.push_str("</svg>");
    svg
}

/// `n` CSS rules in a <style> plus `n` classed nodes. Stresses the simplecss
/// cascade and selector matching.
pub fn css_heavy(n: usize) -> String {
    let mut svg = String::from(HEADER);
    svg.push_str("<style>");
    for i in 0..n {
        let _ = write!(svg, ".c{i} {{ fill: #{:06x}; opacity: 0.9; }}", i & 0xffffff);
    }
    svg.push_str("</style>");
    for i in 0..n {
        let x = (i % 100) as f64 * 2.0;
        let y = (i / 100) as f64 * 2.0;
        let _ = write!(svg, r#"<rect class="c{i}" x="{x}" y="{y}" width="1.5" height="1.5"/>"#);
    }
    svg.push_str("</svg>");
    svg
}

/// One path with `n` cubic segments. Stresses path-data tokenizing.
pub fn path_segments(n: usize) -> String {
    let mut data = String::from("M0 0");
    for i in 0..n {
        let a = (i % 100) as f64;
        let b = ((i + 33) % 100) as f64;
        let c = ((i + 66) % 100) as f64;
        let _ = write!(data, " C{a} {b} {b} {c} {c} {a}");
    }
    format!(r##"{HEADER}<path d="{data}" fill="none" stroke="#000"/></svg>"##)
}

/// Every supported element kind benched individually. Each fixture emits `n`
/// conversion-reachable instances of one kind.
pub const ELEMENT_KINDS: [&str; 22] = [
    "rect", "circle", "ellipse", "line", "polyline", "polygon", "path",
    "g", "use", "symbol", "image",
    "linearGradient", "radialGradient", "pattern",
    "clipPath", "mask",
    "filter", "marker",
    "animate", "animateTransform", "set", "animateMotion",
];

// A 1x1 transparent PNG as a data URL, so `image` exercises the data-url /
// imagesize parse path without meaningful raster decode cost.
const PNG_1X1: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M8AAAMCAQDg6V4kAAAAAElFTkSuQmCC";

/// Generate `n` reachable instances of a single element `kind`.
pub fn single_kind(kind: &str, n: usize) -> String {
    let mut svg = String::from(HEADER);
    let mut body = String::new();
    let mut defs = String::new();

    for i in 0..n {
        let x = (i % 100) as f64 * 2.0;
        let y = (i / 100) as f64 * 2.0;
        match kind {
            "rect" => { let _ = write!(body, r#"<rect x="{x}" y="{y}" width="1.5" height="1.5"/>"#); }
            "circle" => { let _ = write!(body, r#"<circle cx="{x}" cy="{y}" r="1"/>"#); }
            "ellipse" => { let _ = write!(body, r#"<ellipse cx="{x}" cy="{y}" rx="1" ry="0.6"/>"#); }
            "line" => { let _ = write!(body, r##"<line x1="{x}" y1="{y}" x2="{}" y2="{}" stroke="#000"/>"##, x + 1.0, y + 1.0); }
            "polyline" => { let _ = write!(body, r##"<polyline points="{x},{y} {},{y} {},{}" fill="none" stroke="#000"/>"##, x + 1.0, x + 1.0, y + 1.0); }
            "polygon" => { let _ = write!(body, r#"<polygon points="{x},{y} {},{y} {},{}"/>"#, x + 1.0, x + 1.0, y + 1.0); }
            "path" => { let _ = write!(body, r#"<path d="M{x} {y} l1 0 l0 1 z"/>"#); }
            "g" => { let _ = write!(body, r#"<g transform="translate({x} {y})"><rect width="1" height="1"/></g>"#); }
            "use" => {
                if i == 0 { defs.push_str(r#"<rect id="u" width="1" height="1"/>"#); }
                let _ = write!(body, r##"<use href="#u" x="{x}" y="{y}"/>"##);
            }
            "symbol" => {
                if i == 0 { defs.push_str(r#"<symbol id="s" viewBox="0 0 1 1"><rect width="1" height="1"/></symbol>"#); }
                let _ = write!(body, r##"<use href="#s" x="{x}" y="{y}" width="1" height="1"/>"##);
            }
            "image" => { let _ = write!(body, r#"<image x="{x}" y="{y}" width="1" height="1" href="{PNG_1X1}"/>"#); }
            "linearGradient" => {
                let _ = write!(defs, r##"<linearGradient id="lg{i}"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient>"##);
                let _ = write!(body, r##"<rect x="{x}" y="{y}" width="1.5" height="1.5" fill="url(#lg{i})"/>"##);
            }
            "radialGradient" => {
                let _ = write!(defs, r##"<radialGradient id="rg{i}"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></radialGradient>"##);
                let _ = write!(body, r##"<rect x="{x}" y="{y}" width="1.5" height="1.5" fill="url(#rg{i})"/>"##);
            }
            "pattern" => {
                let _ = write!(defs, r#"<pattern id="p{i}" width="1" height="1" patternUnits="userSpaceOnUse"><rect width="1" height="1"/></pattern>"#);
                let _ = write!(body, r##"<rect x="{x}" y="{y}" width="1.5" height="1.5" fill="url(#p{i})"/>"##);
            }
            "clipPath" => {
                let _ = write!(defs, r#"<clipPath id="cp{i}"><rect width="1" height="1"/></clipPath>"#);
                let _ = write!(body, r##"<rect x="{x}" y="{y}" width="1.5" height="1.5" clip-path="url(#cp{i})"/>"##);
            }
            "mask" => {
                let _ = write!(defs, r##"<mask id="m{i}"><rect width="2" height="2" fill="#fff"/></mask>"##);
                let _ = write!(body, r##"<rect x="{x}" y="{y}" width="1.5" height="1.5" mask="url(#m{i})"/>"##);
            }
            "filter" => {
                let _ = write!(defs, r#"<filter id="f{i}"><feGaussianBlur stdDeviation="0.3"/></filter>"#);
                let _ = write!(body, r##"<rect x="{x}" y="{y}" width="1.5" height="1.5" filter="url(#f{i})"/>"##);
            }
            "marker" => {
                let _ = write!(defs, r#"<marker id="mk{i}" markerWidth="1" markerHeight="1"><rect width="1" height="1"/></marker>"#);
                let _ = write!(body, r##"<path d="M{x} {y} l2 0" stroke="#000" marker-end="url(#mk{i})"/>"##);
            }
            "animate" => { let _ = write!(body, r#"<rect x="{x}" y="{y}" width="1.5" height="1.5"><animate attributeName="opacity" from="0" to="1" dur="1s"/></rect>"#); }
            "animateTransform" => { let _ = write!(body, r#"<g transform="translate({x} {y})"><rect width="1.5" height="1.5"><animateTransform attributeName="transform" type="rotate" from="0" to="360" dur="1s"/></rect></g>"#); }
            "set" => { let _ = write!(body, r#"<rect x="{x}" y="{y}" width="1.5" height="1.5"><set attributeName="opacity" to="0.5" begin="0s"/></rect>"#); }
            "animateMotion" => { let _ = write!(body, r#"<rect width="1.5" height="1.5"><animateMotion path="M{x} {y} l1 1" dur="1s"/></rect>"#); }
            other => panic!("unknown element kind: {other}"),
        }
    }

    if !defs.is_empty() {
        let _ = write!(svg, "<defs>{defs}</defs>");
    }
    svg.push_str(&body);
    svg.push_str("</svg>");
    svg
}
