"""Checks that timing comparisons do not silently accept changed output."""

import unittest

from compare_parsers import validate


class OutputChecks(unittest.TestCase):
    def setUp(self):
        self.reference = {
            "first_tick": 1,
            "last_tick": 10000,
            "ticks": 10000,
            "seconds": 2.0,
            "samples": {"10000": {"field_hash": 123}},
        }

    def test_elapsed_time_does_not_change_output_identity(self):
        validate(self.reference, {**self.reference, "seconds": 1.0})

    def test_equal_tick_counts_do_not_hide_changed_fields(self):
        changed = {**self.reference, "samples": {"10000": {"field_hash": 456}}}
        with self.assertRaisesRegex(ValueError, "sampled state differs"):
            validate(self.reference, changed)
        validate(self.reference, changed, allow_differences=True)

    def test_output_override_does_not_allow_incomplete_playback(self):
        with self.assertRaisesRegex(ValueError, "completed ticks differ"):
            validate(
                self.reference,
                {**self.reference, "last_tick": 9999},
                allow_differences=True,
            )


if __name__ == "__main__":
    unittest.main()
