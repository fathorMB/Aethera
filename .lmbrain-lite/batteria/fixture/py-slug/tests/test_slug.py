import unittest

from textkit import slugify


class SlugifyTest(unittest.TestCase):
    def test_simple_title(self):
        self.assertEqual(slugify("Hello World"), "hello-world")

    def test_accents_are_removed(self):
        self.assertEqual(slugify("Café au lait"), "cafe-au-lait")

    def test_runs_of_symbols_collapse(self):
        self.assertEqual(slugify("Rust -- and -- Python!"), "rust-and-python")

    def test_max_len_does_not_leave_a_trailing_separator(self):
        self.assertEqual(slugify("hello world again", max_len=6), "hello")


if __name__ == "__main__":
    unittest.main()
