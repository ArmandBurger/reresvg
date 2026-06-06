// Pure logic for the benchmark dashboard: name parsing, derived metrics, axis
// fitting, and building the render model. It accepts the benchmark data object
// (e.g. window.BENCHMARK_DATA in the browser) as an explicit argument. No DOM
// access — every export is pure, so it is unit-tested with node.

export const SERIES_SEPARATOR = " — ";

// Raw metric keys exactly as they appear after the separator in a series name,
// in display order.
export const RAW_METRICS = [
  { key: "Instructions", label: "Instructions", unit: "instructions" },
  { key: "L1 Hits", label: "L1 Hits", unit: "hits" },
  { key: "LL Hits", label: "LL Hits", unit: "hits" },
  { key: "RAM Hits", label: "RAM Hits", unit: "hits" },
  { key: "Estimated Cycles", label: "Estimated Cycles", unit: "cycles" },
];

// Derived metric definitions, in display order. Each is computed from a
// benchmark's raw counts plus its element count (null when not applicable).
export const DERIVED_METRICS = [
  { key: "cacheMissRate", label: "Cache miss rate %", unit: "%", hint: "(LL+RAM)/total · lower = neater", needsElementCount: false },
  { key: "ramMissRate", label: "RAM miss rate %", unit: "%", hint: "RAM/total · the 35x misses", needsElementCount: false },
  { key: "instructionsPerElement", label: "Instructions / element", unit: "instructions", hint: "Instructions / N", needsElementCount: true },
  { key: "cyclesPerElement", label: "Cycles / element", unit: "cycles", hint: "Estimated Cycles / N", needsElementCount: true },
];

// Map a raw metric label to the field it populates in a rawCounts object.
const RAW_METRIC_FIELD = {
  "Instructions": "instructions",
  "L1 Hits": "l1Hits",
  "LL Hits": "llHits",
  "RAM Hits": "ramHits",
  "Estimated Cycles": "estimatedCycles",
};

// Element counts used by the iai benches, keyed by benchmark group prefix.
const ELEMENT_COUNT_BY_GROUP = {
  scaled: 1000,
  per_element_kind: 200,
};

// Split "scaled/path — Instructions" into { benchmark, metric }, or null.
export function parseSeriesName(name) {
  const index = name.indexOf(SERIES_SEPARATOR);
  if (index === -1) {
    return null;
  }
  return {
    benchmark: name.slice(0, index),
    metric: name.slice(index + SERIES_SEPARATOR.length),
  };
}

// "scaled/path" -> 1000, "per_element_kind/rect" -> 200, unknown -> null.
export function elementCountForBenchmark(benchmark) {
  const group = benchmark.split("/")[0];
  const count = ELEMENT_COUNT_BY_GROUP[group];
  return count === undefined ? null : count;
}

// Compute the four derived metrics from a benchmark's raw counts. The two
// per-element metrics are null when elementCount is null.
export function deriveMetrics(rawCounts, elementCount) {
  const totalAccesses = rawCounts.l1Hits + rawCounts.llHits + rawCounts.ramHits;
  // When cache counters are absent (total 0), report 0% rather than NaN.
  const cacheMissRate = totalAccesses === 0
    ? 0
    : ((rawCounts.llHits + rawCounts.ramHits) / totalAccesses) * 100;
  const ramMissRate = totalAccesses === 0
    ? 0
    : (rawCounts.ramHits / totalAccesses) * 100;
  return {
    cacheMissRate,
    ramMissRate,
    instructionsPerElement: elementCount === null ? null : rawCounts.instructions / elementCount,
    cyclesPerElement: elementCount === null ? null : rawCounts.estimatedCycles / elementCount,
  };
}

// Tight auto-fit Y bounds: [min - pad, max + pad] with pad a small fraction of
// the span. Identical values fall back to a tiny epsilon band so the axis never
// collapses; an empty input returns a safe default.
export function fitYAxis(values) {
  if (values.length === 0) {
    return { min: 0, max: 1 };
  }
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min;
  if (span === 0) {
    const epsilon = (Math.abs(max) || 1) * 1e-6;
    return { min: max - epsilon, max: max + epsilon };
  }
  const pad = span * 0.1;
  return { min: min - pad, max: max + pad };
}

// Reshape one run's flat benches list into { benchmark: rawCounts }.
export function rawCountsByBenchmark(benches) {
  const result = {};
  for (const bench of benches) {
    if (bench == null || typeof bench.name !== "string") {
      continue;
    }
    const parsed = parseSeriesName(bench.name);
    if (parsed === null) {
      continue;
    }
    const field = RAW_METRIC_FIELD[parsed.metric];
    if (field === undefined) {
      continue;
    }
    if (result[parsed.benchmark] === undefined) {
      result[parsed.benchmark] = { instructions: 0, l1Hits: 0, llHits: 0, ramHits: 0, estimatedCycles: 0 };
    }
    result[parsed.benchmark][field] = typeof bench.value === "number" ? bench.value : 0;
  }
  return result;
}

function shortCommitId(commitId) {
  return typeof commitId === "string" ? commitId.slice(0, 7) : "";
}

function commitIdOf(run) {
  return shortCommitId(run.commit && run.commit.id);
}

// Build the full ordered render model: derived sections first, then raw, each a
// list of per-benchmark charts with points across commit-runs and fitted axes.
export function buildDashboardModel(benchmarkData, suiteName) {
  const runs = (benchmarkData && benchmarkData.entries && benchmarkData.entries[suiteName]) || [];
  const perRunCounts = runs.map((run) => rawCountsByBenchmark(run.benches || []));

  const benchmarkSet = new Set();
  for (const counts of perRunCounts) {
    for (const benchmark of Object.keys(counts)) {
      benchmarkSet.add(benchmark);
    }
  }
  const benchmarks = [...benchmarkSet].sort();

  const sections = [];

  for (const metric of DERIVED_METRICS) {
    const charts = [];
    for (const benchmark of benchmarks) {
      const elementCount = elementCountForBenchmark(benchmark);
      if (metric.needsElementCount && elementCount === null) {
        continue;
      }
      const points = [];
      for (let runIndex = 0; runIndex < runs.length; runIndex++) {
        const counts = perRunCounts[runIndex][benchmark];
        if (counts === undefined) {
          continue;
        }
        const value = deriveMetrics(counts, elementCount)[metric.key];
        if (value === null) {
          continue;
        }
        points.push({ commitId: commitIdOf(runs[runIndex]), value });
      }
      if (points.length === 0) {
        continue;
      }
      charts.push({ benchmark, points, axis: fitYAxis(points.map((point) => point.value)) });
    }
    sections.push({ key: metric.key, label: metric.label, unit: metric.unit, hint: metric.hint, isDerived: true, charts });
  }

  for (const metric of RAW_METRICS) {
    const field = RAW_METRIC_FIELD[metric.key];
    const charts = [];
    for (const benchmark of benchmarks) {
      const points = [];
      for (let runIndex = 0; runIndex < runs.length; runIndex++) {
        const counts = perRunCounts[runIndex][benchmark];
        if (counts === undefined) {
          continue;
        }
        points.push({ commitId: commitIdOf(runs[runIndex]), value: counts[field] });
      }
      if (points.length === 0) {
        continue;
      }
      charts.push({ benchmark, points, axis: fitYAxis(points.map((point) => point.value)) });
    }
    sections.push({ key: metric.key, label: metric.label, unit: metric.unit, hint: "", isDerived: false, charts });
  }

  return { suiteName, runCount: runs.length, sections };
}
