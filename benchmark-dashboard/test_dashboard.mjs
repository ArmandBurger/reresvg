// Dependency-free tests for dashboard-logic.mjs. Run with: node test_dashboard.mjs
import assert from "node:assert/strict";
import {
  parseSeriesName,
  elementCountForBenchmark,
  deriveMetrics,
  fitYAxis,
  rawCountsByBenchmark,
  buildDashboardModel,
} from "./dashboard-logic.mjs";

function testParseSeriesName() {
  assert.deepEqual(parseSeriesName("scaled/path — Instructions"), {
    benchmark: "scaled/path",
    metric: "Instructions",
  });
  assert.deepEqual(parseSeriesName("per_element_kind/rect — RAM Hits"), {
    benchmark: "per_element_kind/rect",
    metric: "RAM Hits",
  });
  assert.equal(parseSeriesName("no-separator-here"), null);
}

function testElementCountForBenchmark() {
  assert.equal(elementCountForBenchmark("scaled/path"), 1000);
  assert.equal(elementCountForBenchmark("per_element_kind/rect"), 200);
  assert.equal(elementCountForBenchmark("mystery/thing"), null);
}

function testDeriveMetrics() {
  const rawCounts = {
    instructions: 20000,
    l1Hits: 9900,
    llHits: 60,
    ramHits: 40,
    estimatedCycles: 12000,
  };
  // total accesses = 10000; cache miss = (60+40)/10000*100 = 1.0; ram miss = 0.4
  const derived = deriveMetrics(rawCounts, 200);
  assert.equal(derived.cacheMissRate, 1.0);
  assert.equal(derived.ramMissRate, 0.4);
  assert.equal(derived.instructionsPerElement, 100);
  assert.equal(derived.cyclesPerElement, 60);

  const derivedWithoutElementCount = deriveMetrics(rawCounts, null);
  assert.equal(derivedWithoutElementCount.instructionsPerElement, null);
  assert.equal(derivedWithoutElementCount.cyclesPerElement, null);
  assert.equal(derivedWithoutElementCount.cacheMissRate, 1.0);
}

function testFitYAxis() {
  const normal = fitYAxis([10, 20]);
  // span 10, pad 1 -> [9, 21]
  assert.equal(normal.min, 9);
  assert.equal(normal.max, 21);

  const flat = fitYAxis([5, 5, 5]);
  assert.ok(flat.min < 5 && flat.max > 5, "zero-span must pad around the value");
  assert.ok(flat.max - flat.min > 0, "zero-span band must be non-empty");

  assert.deepEqual(fitYAxis([]), { min: 0, max: 1 });
}

function testRawCountsByBenchmark() {
  const benches = [
    { name: "scaled/path — Instructions", value: 19, unit: "instructions" },
    { name: "scaled/path — RAM Hits", value: 7, unit: "hits" },
    { name: "scaled/path — Unknown Metric", value: 999, unit: "?" },
    { name: "scaled/flat — Instructions", value: 5, unit: "instructions" },
  ];
  const counts = rawCountsByBenchmark(benches);
  assert.equal(counts["scaled/path"].instructions, 19);
  assert.equal(counts["scaled/path"].ramHits, 7);
  assert.equal(counts["scaled/flat"].instructions, 5);
  assert.equal(counts["scaled/flat"].ramHits, 0);
  // Unknown metric labels are skipped, not added as fields.
  assert.equal(counts["scaled/path"]["Unknown Metric"], undefined);
}

function testBuildDashboardModel() {
  const data = {
    entries: {
      suite: [
        {
          commit: { id: "abcdef1234567" },
          benches: [
            { name: "scaled/path — Instructions", value: 1000 },
            { name: "scaled/path — L1 Hits", value: 990 },
            { name: "scaled/path — LL Hits", value: 6 },
            { name: "scaled/path — RAM Hits", value: 4 },
            { name: "scaled/path — Estimated Cycles", value: 1200 },
          ],
        },
      ],
    },
  };
  const model = buildDashboardModel(data, "suite");
  assert.equal(model.runCount, 1);
  assert.equal(model.sections.length, 9);
  assert.equal(model.sections[0].key, "cacheMissRate");
  assert.equal(model.sections[0].isDerived, true);
  assert.equal(model.sections[8].label, "Estimated Cycles");
  const cacheChart = model.sections[0].charts.find(
    (chart) => chart.benchmark === "scaled/path",
  );
  // total = 990+6+4 = 1000; (6+4)/1000*100 = 1.0
  assert.equal(cacheChart.points[0].value, 1.0);
  assert.equal(cacheChart.points[0].commitId, "abcdef1");
}

function testBuildDashboardModelMultiRun() {
  function makeRun(commitId, pathInstructions, includeFlat) {
    const benches = [
      { name: "scaled/path — Instructions", value: pathInstructions },
      { name: "scaled/path — L1 Hits", value: 990 },
      { name: "scaled/path — LL Hits", value: 6 },
      { name: "scaled/path — RAM Hits", value: 4 },
      { name: "scaled/path — Estimated Cycles", value: 1200 },
    ];
    if (includeFlat) {
      benches.push({ name: "scaled/flat — Instructions", value: 50 });
      benches.push({ name: "scaled/flat — L1 Hits", value: 80 });
      benches.push({ name: "scaled/flat — LL Hits", value: 1 });
      benches.push({ name: "scaled/flat — RAM Hits", value: 1 });
      benches.push({ name: "scaled/flat — Estimated Cycles", value: 90 });
    }
    return { commit: { id: commitId }, benches };
  }
  const data = { entries: { suite: [makeRun("aaaaaaa1111", 1000, false), makeRun("bbbbbbb2222", 1100, true)] } };
  const model = buildDashboardModel(data, "suite");
  assert.equal(model.runCount, 2);
  const instructionsSection = model.sections.find((section) => section.label === "Instructions");
  const pathChart = instructionsSection.charts.find((chart) => chart.benchmark === "scaled/path");
  // present in both runs -> two points, in chronological order
  assert.deepEqual(pathChart.points.map((point) => point.value), [1000, 1100]);
  assert.deepEqual(pathChart.points.map((point) => point.commitId), ["aaaaaaa", "bbbbbbb"]);
  const flatChart = instructionsSection.charts.find((chart) => chart.benchmark === "scaled/flat");
  // present only in the second run -> a single point
  assert.equal(flatChart.points.length, 1);
  assert.equal(flatChart.points[0].value, 50);
  assert.equal(flatChart.points[0].commitId, "bbbbbbb");
}

function runAll() {
  testParseSeriesName();
  testElementCountForBenchmark();
  testDeriveMetrics();
  testFitYAxis();
  testRawCountsByBenchmark();
  testBuildDashboardModel();
  testBuildDashboardModelMultiRun();
  console.log("OK: all dashboard-logic tests passed");
}

runAll();
