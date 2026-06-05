// Criterion wall-clock benchmarks for the usvg parser. Local developer-facing
// profiling; the deterministic CI gate lives in parsing_iai.rs over the same
// generators.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

#[path = "generators.rs"]
mod generators;

use generators::{
    css_heavy, flat_rects, gradient_heavy, nested_groups, path_segments, single_kind, ELEMENT_KINDS,
};

// Count-scaling generators grow the number of sibling nodes; they probe large
// inputs up to ten thousand elements.
const SCALES: [usize; 3] = [100, 1_000, 10_000];

// `nested_groups` grows tree DEPTH, and usvg rejects documents deeper than 1024
// nodes (Error::NodesLimitReached). This generator therefore scales within that
// valid range instead of the shared count scale.
const DEPTH_SCALES: [usize; 3] = [100, 500, 1_000];

fn parse(svg: &str) -> usvg::Tree {
    let options = usvg::Options::default();
    usvg::Tree::from_str(black_box(svg), black_box(&options)).expect("generated SVG must parse")
}

fn scaled_group(
    criterion: &mut Criterion,
    name: &str,
    scales: &[usize],
    generate: impl Fn(usize) -> String,
) {
    let mut group = criterion.benchmark_group(name);
    for &n in scales {
        let svg = generate(n); // built once, outside the timed closure
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &svg, |bencher, svg| {
            bencher.iter(|| parse(svg));
        });
    }
    group.finish();
}

fn scaled_benchmarks(criterion: &mut Criterion) {
    scaled_group(criterion, "parse/flat_rects", &SCALES, flat_rects);
    scaled_group(criterion, "parse/nested_groups", &DEPTH_SCALES, nested_groups);
    scaled_group(criterion, "parse/gradient_heavy", &SCALES, gradient_heavy);
    scaled_group(criterion, "parse/css_heavy", &SCALES, css_heavy);
    scaled_group(criterion, "parse/path_segments", &SCALES, path_segments);
}

fn per_element_benchmarks(criterion: &mut Criterion) {
    const N: usize = 1_000;
    let mut group = criterion.benchmark_group("parse/per_element_kind");
    group.throughput(Throughput::Elements(N as u64));
    for kind in ELEMENT_KINDS {
        let svg = single_kind(kind, N);
        group.bench_with_input(BenchmarkId::from_parameter(kind), &svg, |bencher, svg| {
            bencher.iter(|| parse(svg));
        });
    }
    group.finish();
}

criterion_group!(benches, scaled_benchmarks, per_element_benchmarks);
criterion_main!(benches);
