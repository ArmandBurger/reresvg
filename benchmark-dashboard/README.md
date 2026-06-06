# Benchmark dashboard

A self-contained static dashboard for the usvg parser benchmark history. It reads
the data file that `benchmark-action/github-action-benchmark` publishes to
`dev/bench/data.js` and renders it grouped by metric, with reinterpreted
(derived) metrics pinned on top and tight auto-fit axes so small changes are
visible.

Live: <https://armandburger.github.io/reresvg/> (the gh-pages root).

## Files

- `dashboard-logic.mjs` — pure data logic (derived metrics, axis fitting,
  element-count inference, model building). No DOM; unit-tested with node.
- `index.html` — inline-CSS page that imports the logic, reads
  `window.BENCHMARK_DATA`, and renders zero-dependency inline-SVG charts plus a
  click-to-enlarge overlay.
- `test_dashboard.mjs` — dependency-free tests for the logic.

## Develop

Run the logic tests (needs node, no other dependencies):

    node benchmark-dashboard/test_dashboard.mjs

The page only renders when served over HTTP with a `dev/bench/data.js` present
(true on gh-pages). Opening `index.html` via `file://` will not load the ES
module — view the published URL instead.

## Deploy

`.github/workflows/deploy-dashboard.yml` runs on pushes to `main` that touch
`benchmark-dashboard/**`. It runs the logic tests, then publishes `index.html`
and `dashboard-logic.mjs` to the `gh-pages` root via plain git (commit authored
by `github-actions[bot]`). The benchmark action keeps owning `dev/bench/`.

## Derived metrics

| Metric | Formula |
|---|---|
| Cache miss rate % | (LL + RAM) / (L1 + LL + RAM) · 100 |
| RAM miss rate % | RAM / (L1 + LL + RAM) · 100 |
| Instructions / element | Instructions / N |
| Cycles / element | Estimated Cycles / N |

N is the element count per benchmark: `scaled/*` = 1000, `per_element_kind/*` =
200. Benchmarks with an unknown group get the two rate metrics but not the
per-element ones.
