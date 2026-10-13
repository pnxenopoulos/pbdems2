"""Output verification and allocation-probe transport."""

import ctypes
import unittest

from allocation_probe import AllocationProbe, Snapshot
from datasets import summarize

try:
    import polars as pl
except ImportError:
    pl = None


def probe_for(snapshot):
    probe = AllocationProbe.__new__(AllocationProbe)
    probe._end = lambda: snapshot
    return probe


class AllocationTests(unittest.TestCase):
    def test_snapshot_has_the_native_abi_size(self):
        self.assertEqual(ctypes.sizeof(Snapshot), 160)

    def test_negative_net_growth_keeps_its_sign(self):
        result = probe_for(Snapshot(live_start=100, live_end=40, peak_live=130)).end()
        self.assertEqual((result["net_live"], result["peak_growth"]), (-60, 30))

    def test_stage_counters_reach_the_result(self):
        snapshot = Snapshot()
        snapshot.stages[1].allocations = 7
        snapshot.stages[1].requested_bytes = 256
        stage = probe_for(snapshot).end()["stages"]["rows"]
        self.assertEqual((stage["allocations"], stage["requested_bytes"]), (7, 256))

    def test_allocation_underflow_is_reported(self):
        with self.assertRaisesRegex(RuntimeError, "underflow"):
            probe_for(Snapshot(underflows=1)).end()


@unittest.skipIf(pl is None, "requires the profiling Python environment with Polars")
class OutputTests(unittest.TestCase):
    def setUp(self):
        self.frame = pl.DataFrame({"tick": [1, 2, 3], "value": ["a", None, "b"]})
        self.original = summarize({"rows": self.frame}, True)["rows"]

    def test_unordered_hash_ignores_row_order(self):
        reversed_rows = summarize({"rows": self.frame.reverse()}, True)["rows"]
        self.assertEqual(self.original["row_hash_sum"], reversed_rows["row_hash_sum"])

    def test_ordered_hash_detects_row_order(self):
        reversed_rows = summarize({"rows": self.frame.reverse()}, True)["rows"]
        self.assertNotEqual(
            self.original["ordered_row_hash_sum"], reversed_rows["ordered_row_hash_sum"]
        )

    def test_row_hash_detects_changed_values(self):
        changed = self.frame.with_columns(pl.lit("changed").alias("value"))
        self.assertNotEqual(
            self.original["row_hash_sum"],
            summarize({"rows": changed}, True)["rows"]["row_hash_sum"],
        )

    def test_summary_detects_changed_schema(self):
        changed = self.frame.with_columns(pl.col("tick").cast(pl.Int32))
        self.assertNotEqual(
            self.original["schema"],
            summarize({"rows": changed}, True)["rows"]["schema"],
        )

    def test_empty_results_are_rejected_by_default(self):
        with self.assertRaisesRegex(RuntimeError, "no rows"):
            summarize({"rows": self.frame.clear()}, True)

    def test_explicitly_allowed_empty_results_keep_the_schema(self):
        result = summarize({"rows": self.frame.clear()}, True, allow_empty=True)["rows"]
        self.assertEqual(
            (
                result["rows"],
                result["columns"],
                result["tick_min"],
                result["ordered_row_hash_sum"],
            ),
            (0, 2, None, 0),
        )


if __name__ == "__main__":
    unittest.main()
