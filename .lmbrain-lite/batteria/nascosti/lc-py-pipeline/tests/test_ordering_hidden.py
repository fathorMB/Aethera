import unittest

from pipeline.util.ordering import rank_totals


class RankTotalsHiddenTest(unittest.TestCase):
    def test_ties_broken_alphabetically_regardless_of_insertion_order(self):
        totals = {"zoe": 100, "bob": 100, "ann": 100, "mia": 50}
        self.assertEqual(
            rank_totals(totals),
            [("ann", 100), ("bob", 100), ("zoe", 100), ("mia", 50)],
        )

    def test_single_entry(self):
        self.assertEqual(rank_totals({"a": 5}), [("a", 5)])

    def test_empty(self):
        self.assertEqual(rank_totals({}), [])

    def test_no_ties_still_sorted_by_total(self):
        totals = {"low": 1, "high": 100, "mid": 50}
        self.assertEqual(rank_totals(totals), [("high", 100), ("mid", 50), ("low", 1)])


if __name__ == "__main__":
    unittest.main()
