import unittest

from textkit import wrap


class WrapTest(unittest.TestCase):
    def test_wraps_at_spaces(self):
        self.assertEqual(wrap("the quick brown fox", 10), ["the quick", "brown fox"])

    def test_long_word_stays_whole(self):
        self.assertEqual(wrap("a extraordinarily b", 5), ["a", "extraordinarily", "b"])


if __name__ == "__main__":
    unittest.main()
