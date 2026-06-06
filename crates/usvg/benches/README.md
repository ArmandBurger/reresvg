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

## CI history & regression tracking

On every push to `main` and every pull request, `.github/workflows/bench.yml`
runs the iai-callgrind parser benchmarks with `--save-summary=json`, converts the
per-benchmark `summary.json` files with `iai_to_benchmark_json.py`, and feeds the
result to `benchmark-action/github-action-benchmark`.

- **Push to `main`:** the new instruction-count / cache / cycle numbers are
  appended to the `gh-pages` history and the trend chart regenerates at
  <https://armandburger.github.io/reresvg/dev/bench/>.
- **Pull request:** the run is compared against the latest `main` baseline. Any
  series more than 5% worse triggers an advisory comment. Regressions never fail
  the build (`fail-on-alert: false`); small counters such as RAM/LL hits can swing
  several percent on a one-hit change, so treat those comments as informational.

Five metrics are tracked per benchmark: Instructions, L1 Hits, LL Hits, RAM Hits,
and Estimated Cycles.

### One-time setup (repository owner)

1. Create an empty `gh-pages` branch and push it to `origin`.
2. Settings → Pages → Source: *Deploy from a branch* → `gh-pages` / root.
3. Settings → Actions → General → Workflow permissions → *Read and write
   permissions* (lets `GITHUB_TOKEN` push history and post comments).

History commits are authored by `github-actions[bot]`, never a personal git
identity.

### Updating the converter

`iai_to_benchmark_json.py` reads `callgrind_summary.callgrind_run.total.summary`
from each `summary.json` (iai-callgrind summary schema v3). Its tests run without
extra dependencies:

    python3 crates/usvg/benches/test_iai_converter.py

## Dashboard

A custom dashboard renders this history more legibly than the default page:
grouped by metric, with derived metrics (cache miss rate, RAM miss rate,
instructions/element, cycles/element) pinned on top and tight auto-fit axes.
Source and docs: `benchmark-dashboard/`. Live: <https://armandburger.github.io/reresvg/>.
