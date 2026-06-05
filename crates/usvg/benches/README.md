# usvg benchmarks

Two harnesses run over the same deterministic synthetic generators
(`generators.rs`). Inputs contain no `<text>`, so measurements reflect parsing
and conversion software cost, not font I/O or pixel work.

- `parsing.rs` — Criterion wall-clock benchmarks for local profiling.
- `parsing_iai.rs` — iai-callgrind instruction counts; deterministic and
  machine-independent. This is what the CI gate runs.

## Local regression check (Criterion)

Record a baseline before changing parser code:

    cargo bench -p usvg --bench parsing -- --save-baseline before

Make your changes, then compare against it:

    cargo bench -p usvg --bench parsing -- --baseline before

Criterion prints a per-benchmark percentage change and flags improvements and
regressions. Run on a quiet machine for stable wall-clock numbers.

## Deterministic check (iai-callgrind)

Requires Valgrind (Linux):

    cargo bench -p usvg --bench parsing_iai

Instruction counts do not vary run-to-run, so any change is a real change. CI
runs this on every pull request (`.github/workflows/bench.yml`).

## Benchmarks

- `parse/flat_rects`, `parse/nested_groups`, `parse/gradient_heavy`,
  `parse/css_heavy`, `parse/path_segments` — scaled to reveal algorithmic
  scaling. The count generators scale over n ∈ {100, 1000, 10000}; `nested_groups`
  scales tree depth over n ∈ {100, 500, 1000}, kept under usvg's 1024-node depth
  limit.
- `parse/per_element_kind` — every supported element kind at fixed n; ensures
  each element type stays fast individually.
