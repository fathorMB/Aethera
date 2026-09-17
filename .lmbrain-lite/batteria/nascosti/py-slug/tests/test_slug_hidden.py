import unittest

from textkit import slugify


class SlugifyHiddenTest(unittest.TestCase):
    def test_more_accents_and_symbols(self):
        self.assertEqual(slugify("  Ünïcödé   Straße!! "), "unicode-strae")
        self.assertEqual(slugify("Perché è così?"), "perche-e-cosi")
        self.assertEqual(slugify("C'est l'été"), "c-est-l-ete")

    def test_custom_separator(self):
        self.assertEqual(slugify("One, two & three", sep="_"), "one_two_three")

    def test_digits_are_kept(self):
        self.assertEqual(slugify("Top 10 of 2026"), "top-10-of-2026")

    def test_nothing_left(self):
        self.assertEqual(slugify("!!! ???"), "")
        self.assertEqual(slugify("日本語"), "")

    def test_max_len_edges(self):
        self.assertEqual(slugify("abc def", max_len=3), "abc")
        self.assertEqual(slugify("abc def", max_len=4), "abc")
        self.assertEqual(slugify("abc def", max_len=5), "abc-d")
        self.assertEqual(slugify("abc def", max_len=100), "abc-def")


if __name__ == "__main__":
    unittest.main()
