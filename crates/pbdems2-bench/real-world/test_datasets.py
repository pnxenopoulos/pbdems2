"""Small checks for output verification and allocation-probe transport."""
import ctypes
import unittest

from allocation_probe import AllocationProbe, Snapshot
from datasets import summarize

try:
    import polars as pl
except ImportError:
    pl = None


class AllocationTests(unittest.TestCase):
    def test_snapshot_layout_and_negative_net_growth(self):
        self.assertEqual(ctypes.sizeof(Snapshot), 160)
        snapshot = Snapshot(live_start=100, live_end=40, peak_live=130)
        snapshot.stages[1].allocations = 7
        snapshot.stages[1].requested_bytes = 256
        probe = AllocationProbe.__new__(AllocationProbe)
        probe._end = lambda: snapshot
        result = probe.end()
        self.assertEqual((result["net_live"], result["peak_growth"]), (-60, 30))
        self.assertEqual(result["stages"]["rows"]["allocations"], 7)
        self.assertEqual(result["stages"]["rows"]["requested_bytes"], 256)
        snapshot.underflows = 1
        with self.assertRaisesRegex(RuntimeError, "underflow"):
            probe.end()


@unittest.skipIf(pl is None, "requires the profiling Python environment with Polars")
class OutputTests(unittest.TestCase):
    def test_value_order_and_schema_changes_are_distinguished(self):
        frame = pl.DataFrame({"tick": [1, 2, 3], "value": ["a", None, "b"]})
        original = summarize({"rows": frame}, True)["rows"]
        reversed_rows = summarize({"rows": frame.reverse()}, True)["rows"]
        self.assertEqual(original["row_hash_sum"], reversed_rows["row_hash_sum"])
        self.assertNotEqual(original["ordered_row_hash_sum"], reversed_rows["ordered_row_hash_sum"])
        altered = summarize({"rows": frame.with_columns(pl.lit("changed").alias("value"))}, True)["rows"]
        self.assertNotEqual(original["row_hash_sum"], altered["row_hash_sum"])
        typed = summarize({"rows": frame.with_columns(pl.col("tick").cast(pl.Int32))}, True)["rows"]
        self.assertNotEqual(original["schema"], typed["schema"])

    def test_empty_results_require_an_explicit_allowance(self):
        frame = pl.DataFrame(schema={"tick": pl.Int32, "value": pl.String})
        with self.assertRaisesRegex(RuntimeError, "no rows"):
            summarize({"rows": frame}, True)
        result = summarize({"rows": frame}, True, allow_empty=True)["rows"]
        self.assertEqual((result["rows"], result["columns"]), (0, 2))
        self.assertIsNone(result["tick_min"])
        self.assertEqual(result["ordered_row_hash_sum"], 0)


if __name__ == "__main__":
    unittest.main()

