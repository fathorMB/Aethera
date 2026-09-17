import unittest

from intervals import gaps, merge, total_length


class IntervalsHiddenTest(unittest.TestCase):
    def test_merge_edges(self):
        self.assertEqual(merge([]), [])
        self.assertEqual(merge([(3, 3), (5, 5)]), [])
        self.assertEqual(merge([(1, 10), (2, 3)]), [(1, 10)])
        self.assertEqual(merge([(-5, -1), (-1, 0), (2, 3)]), [(-5, 0), (2, 3)])
        self.assertEqual(merge([(1, 2), (1, 2)]), [(1, 2)])

    def test_merge_does_not_modify_input(self):
        data = [(4, 6), (1, 2)]
        merge(data)
        self.assertEqual(data, [(4, 6), (1, 2)])

    def test_merge_returns_tuples(self):
        self.assertEqual(merge([[1, 3], [2, 5]]), [(1, 5)])
        self.assertIsInstance(merge([[1, 3]])[0], tuple)

    def test_total_length_edges(self):
        self.assertEqual(total_length([]), 0)
        self.assertEqual(total_length([(2, 2)]), 0)
        self.assertEqual(total_length([(0, 3), (0, 3), (1, 2)]), 3)
        with self.assertRaises(ValueError):
            total_length([(3, 1)])

    def test_gaps_edges(self):
        self.assertEqual(gaps([], 0, 3), [(0, 3)])
        self.assertEqual(gaps([(-10, 10)], 0, 3), [])
        self.assertEqual(gaps([(-10, 1), (2, 99)], 0, 5), [(1, 2)])
        self.assertEqual(gaps([(0, 1)], 5, 5), [])
        self.assertEqual(gaps([(20, 30)], 0, 5), [(0, 5)])
        self.assertEqual(gaps([(1, 2), (2, 3)], 0, 4), [(0, 1), (3, 4)])
        with self.assertRaises(ValueError):
            gaps([], 4, 1)


if __name__ == "__main__":
    unittest.main()
