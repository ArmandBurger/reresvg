#!/usr/bin/env python3
"""Tests for iai_to_benchmark_json. Dependency-free: run with `python3`."""

import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import iai_to_benchmark_json as converter

FIXTURE_ROOT = Path(__file__).resolve().parent / "testdata" / "iai_summary"


def test_entries_from_committed_fixtures():
    entries = converter.build_benchmark_entries(FIXTURE_ROOT)
    # Two fixture summaries, each contributing one entry per tracked metric.
    expected_entry_count = 2 * len(converter.TRACKED_METRICS)
    assert len(entries) == expected_entry_count, (
        f"expected {expected_entry_count} entries, got {len(entries)}"
    )
    by_name = {entry["name"]: entry for entry in entries}
    assert by_name["scaled/flat — Instructions"] == {
        "name": "scaled/flat — Instructions",
        "unit": "instructions",
        "value": 1234567,
    }
    assert by_name["scaled/flat — Estimated Cycles"]["unit"] == "cycles"
    assert by_name["per_element_kind/rect — RAM Hits"]["value"] == 91
    assert by_name["per_element_kind/rect — RAM Hits"]["unit"] == "hits"
    names = [entry["name"] for entry in entries]
    assert names == sorted(names), "entries must be sorted by name"


def test_current_metric_value_variants():
    # Fresh run, no baseline.
    assert converter.current_metric_value({"Left": 100}) == 100
    # Baseline present: [current, old] — the hot path after the first CI run.
    assert converter.current_metric_value({"Both": [555, 444]}) == 555
    # Only an old value: documented fallback, returns it.
    assert converter.current_metric_value({"Right": 99}) == 99
    # Unrecognized shape is a hard error.
    try:
        converter.current_metric_value({"Unknown": 0})
    except ValueError:
        return
    raise AssertionError("expected ValueError for unrecognized metric node")


def test_missing_summaries_raises():
    with tempfile.TemporaryDirectory() as empty_root:
        try:
            converter.build_benchmark_entries(empty_root)
        except ValueError as error:
            assert "no summary.json" in str(error)
            return
        raise AssertionError("expected ValueError for empty input root")


def run_all():
    test_entries_from_committed_fixtures()
    test_current_metric_value_variants()
    test_missing_summaries_raises()
    print("OK: all converter tests passed")


if __name__ == "__main__":
    run_all()
