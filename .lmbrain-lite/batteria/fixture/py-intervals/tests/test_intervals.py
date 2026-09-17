import unittest

from intervals import gaps, merge, total_length


class IntervalsTest(unittest.TestCase):
    def test_merge_overlapping_and_touching(self):
        self.assertEqual(merge([(5, 8), (1, 3), (3, 4), (7, 10)]), [(1, 4), (5, 10)])

    def test_merge_rejects_invalid(self):
        with self.assertRaises(ValueError):
            merge([(1, 2), (4, 3)])

    def test_total_length(self):
        self.assertEqual(total_length([(0, 5), (3, 8), (10, 11)]), 9)

    def test_gaps(self):
        self.assertEqual(gaps([(2, 4), (6, 7)], 0, 8), [(0, 2), (4, 6), (7, 8)])


if __name__ == "__main__":
    unittest.main()
