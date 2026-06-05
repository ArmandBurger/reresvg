#!/usr/bin/env python3
"""Convert iai-callgrind summary.json files into the JSON array consumed by
benchmark-action/github-action-benchmark (customSmallerIsBetter tool).

Reads every summary.json under an input root (default: target/iai), extracts the
total callgrind metrics for each benchmark, and writes a flat JSON array of
{name, unit, value} objects. One object per (benchmark, metric) pair, so each
metric becomes its own series/chart in the benchmark dashboard.
"""

import argparse
import json
import sys
from pathlib import Path

# iai-callgrind event-kind keys (summary.v3 schema) mapped to the human-readable
# metric label used in the series name and the unit reported to the chart.
TRACKED_METRICS = [
    ("Ir", "Instructions", "instructions"),
    ("L1hits", "L1 Hits", "hits"),
    ("LLhits", "LL Hits", "hits"),
    ("RamHits", "RAM Hits", "hits"),
    ("EstimatedCycles", "Estimated Cycles", "cycles"),
]


def current_metric_value(metrics_either_or_both):
    """Return the current-run value from an EitherOrBoth_for_uint64 node.

    A fresh run without a stored baseline serializes as {"Left": value}; when a
    baseline is present it is {"Both": [current, old]}. {"Right": value} means
    only an old value exists, which should not happen for a current run.
    """
    if "Left" in metrics_either_or_both:
        return metrics_either_or_both["Left"]
    if "Both" in metrics_either_or_both:
        return metrics_either_or_both["Both"][0]
    if "Right" in metrics_either_or_both:
        return metrics_either_or_both["Right"]
    raise ValueError(f"unrecognized metric node: {metrics_either_or_both!r}")


def series_name(summary, metric_label):
    """Build a stable chart-series key from the benchmark identity.

    Combines the bench function name with the per-bench id, yielding names like
    "scaled/flat — Instructions".
    """
    function_name = summary["function_name"]
    benchmark_id = summary.get("id")
    if benchmark_id is None:
        location = function_name
    else:
        location = f"{function_name}/{benchmark_id}"
    return f"{location} — {metric_label}"


def entries_from_summary(summary):
    """Map one iai-callgrind summary to one benchmark entry per tracked metric.

    Metrics live at callgrind_summary.callgrind_run.total.summary (schema v3),
    keyed by event kind. Event kinds absent from the summary are skipped.
    """
    event_metrics = summary["callgrind_summary"]["callgrind_run"]["total"]["summary"]
    entries = []
    for event_kind, metric_label, unit in TRACKED_METRICS:
        metric_diff = event_metrics.get(event_kind)
        if metric_diff is None:
            continue
        value = current_metric_value(metric_diff["metrics"])
        entries.append(
            {
                "name": series_name(summary, metric_label),
                "unit": unit,
                "value": value,
            }
        )
    return entries


def build_benchmark_entries(input_root):
    """Convert every summary.json under input_root into sorted benchmark entries.

    Raises ValueError when no summary.json files are found, so a silent format or
    path change cannot produce an empty, falsely-passing history point.
    """
    summary_paths = sorted(Path(input_root).rglob("summary.json"))
    if not summary_paths:
        raise ValueError(f"no summary.json files found under {input_root}")
    entries = []
    for summary_path in summary_paths:
        summary = json.loads(summary_path.read_text())
        entries.extend(entries_from_summary(summary))
    entries.sort(key=lambda entry: entry["name"])
    return entries


def main(argument_list=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input-root", default="target/iai")
    parser.add_argument("--output", default="output.json")
    arguments = parser.parse_args(argument_list)
    try:
        entries = build_benchmark_entries(arguments.input_root)
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    Path(arguments.output).write_text(json.dumps(entries, indent=2) + "\n")
    print(f"wrote {len(entries)} benchmark entries to {arguments.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
