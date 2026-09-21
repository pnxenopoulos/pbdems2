"""Checks for sample alignment, weighting, and inclusive attribution."""
import unittest

from analyze import json_values, perf_periods, summarize


def profile():
    names = ["pbdems2::playback::run", "awpy::datasets::Parser::player_states",
             "_awpy::states_to_frame"]
    return {"threads": [{
        "samples": {"time": [0, 0.001], "stack": [1, 2]},
        "stackTable": {"prefix": [None, 0, 1], "frame": [0, 1, 2]},
        "frameTable": {"func": [0, 1, 2]},
        "funcTable": {"name": [0, 1, 2], "resource": [-1, -1, -1]},
        "stringArray": names,
    }]}


class AnalysisTests(unittest.TestCase):
    def test_perf_timestamps_and_periods(self):
        self.assertEqual(perf_periods("  python 42/42 12.000000345: 7 cpu-clock:u:"),
                         {12_000_000_345: 7})
        with self.assertRaises(ValueError):
            perf_periods("no samples")

    def test_json_stream_and_array(self):
        self.assertEqual(list(json_values('{"x":1}\n{"x":2}')), [{"x": 1}, {"x": 2}])
        self.assertEqual(list(json_values('[{"x":1}]')), [{"x": 1}])

    def test_period_weighting_and_overlapping_groups(self):
        result, folded = summarize(profile(), {1000: 100, 2000: 300}, [(1000, 3000)], {})
        self.assertEqual(result["samples"], 2)
        self.assertEqual(result["inclusive_percent"]["awpy_snapshot_rows"], 100)
        self.assertEqual(result["inclusive_percent"]["dataframe"], 75)
        self.assertEqual(result["disjoint_stage_percent"], {"snapshot_rows": 25, "dataframe": 75})
        self.assertEqual(sum(folded.values()), 400)
        self.assertTrue(all(stack.startswith("pbdems2::playback::run;") for stack in folded))

    def test_half_open_windows_exclude_output_inspection(self):
        result, _ = summarize(profile(), {1000: 100, 2000: 300}, [(1000, 2000)], {})
        self.assertEqual(result["samples"], 1)
        self.assertEqual(result["inclusive_percent"]["dataframe"], 0)

    def test_column_accumulation_and_final_conversion_are_separate_stages(self):
        recorded = profile()
        recorded["threads"][0]["stringArray"][1:] = [
            "_awpy::snapshot_columns::SnapshotColumns::push",
            "_awpy::snapshot_columns::SnapshotColumns::into_frame",
        ]
        result, _ = summarize(recorded, {1000: 100, 2000: 300}, [(1000, 3000)], {})
        self.assertEqual(result["inclusive_percent"]["awpy_snapshot_columns"], 100)
        self.assertEqual(result["disjoint_stage_percent"],
                         {"snapshot_columns": 25, "dataframe": 75})

    def test_timestamp_mismatch_is_not_silently_accepted(self):
        with self.assertRaisesRegex(ValueError, "mismatch"):
            summarize(profile(), {1000: 100, 2001: 300}, [(0, 3000)], {})
        with self.assertRaisesRegex(ValueError, "dropped"):
            summarize(profile(), {1000: 100, 2000: 300, 3000: 100}, [(0, 4000)], {})

    def test_empty_window_is_an_error(self):
        with self.assertRaisesRegex(ValueError, "no samples"):
            summarize(profile(), {1000: 100, 2000: 300}, [(3000, 4000)], {})


if __name__ == "__main__":
    unittest.main()
