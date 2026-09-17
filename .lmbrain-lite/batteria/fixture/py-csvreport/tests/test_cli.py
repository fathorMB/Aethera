import io
import json
import os
import tempfile
import unittest

from salesreport.cli import main

CSV = "region,product,amount\nnorth,pen,1.50\nsouth,pen,2.005\nnorth,ink,10\n\n"


class CliTest(unittest.TestCase):
    def setUp(self):
        fd, self.path = tempfile.mkstemp(suffix=".csv")
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write(CSV)

    def tearDown(self):
        os.remove(self.path)

    def run_cli(self, *args):
        out = io.StringIO()
        code = main([self.path, *args], stdout=out)
        return code, out.getvalue()

    def test_text_by_region(self):
        code, out = self.run_cli()
        self.assertEqual(code, 0)
        self.assertEqual(
            out,
            "north           2        11.50\n"
            "south           1         2.01\n"
            "TOTAL           3        13.51\n",
        )

    def test_json_by_product(self):
        code, out = self.run_cli("--by", "product", "--format", "json")
        self.assertEqual(code, 0)
        self.assertEqual(
            json.loads(out),
            {
                "groups": [
                    {"key": "ink", "count": 1, "total": "10.00"},
                    {"key": "pen", "count": 2, "total": "3.51"},
                ],
                "count": 3,
                "grand_total": "13.51",
            },
        )


if __name__ == "__main__":
    unittest.main()
