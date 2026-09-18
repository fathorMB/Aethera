import unittest

from pipeline.writers.fixed_width_writer import render_top_customers_fixed_width
from pipeline.writers.json_writer import render_top_customers_json
from pipeline.writers.markdown_writer import render_top_customers_markdown


class MarkdownWriterTest(unittest.TestCase):
    def test_renders_ranked_rows(self):
        table = render_top_customers_markdown([("Ann", 1000), ("Bob", 500)], "EUR")
        lines = table.splitlines()
        self.assertEqual(lines[0], "| # | customer | total |")
        self.assertIn("| 1 | Ann | 10.00 EUR |", lines)
        self.assertIn("| 2 | Bob | 5.00 EUR |", lines)

    def test_empty_rows_still_has_header(self):
        table = render_top_customers_markdown([], "EUR")
        self.assertEqual(table.splitlines()[0], "| # | customer | total |")


class JsonWriterTest(unittest.TestCase):
    def test_renders_ranking(self):
        import json

        text = render_top_customers_json([("c1", "Ann", 1000)], "EUR")
        payload = json.loads(text)
        self.assertEqual(payload["currency"], "EUR")
        self.assertEqual(payload["ranking"], [{"customer_id": "c1", "name": "Ann", "total": "10.00"}])


class FixedWidthWriterTest(unittest.TestCase):
    def test_columns_aligned(self):
        text = render_top_customers_fixed_width([("Ann", 1000)])
        self.assertEqual(text, "Ann                            10.00\n")

    def test_empty_rows_is_empty_string(self):
        self.assertEqual(render_top_customers_fixed_width([]), "")


if __name__ == "__main__":
    unittest.main()
