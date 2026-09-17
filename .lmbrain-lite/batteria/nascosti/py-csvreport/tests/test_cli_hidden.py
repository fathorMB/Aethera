import contextlib
import io
import json
import os
import tempfile
import unittest

from salesreport.cli import main


class CliHiddenTest(unittest.TestCase):
    def write(self, text):
        fd, path = tempfile.mkstemp(suffix=".csv")
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write(text)
        self.addCleanup(os.remove, path)
        return path

    def test_json_by_region_with_ties(self):
        path = self.write("region,product,amount\nb,x,1\na,y,1\nc,z,-0.5\n")
        out = io.StringIO()
        self.assertEqual(main([path, "--format", "json"], stdout=out), 0)
        self.assertTrue(out.getvalue().endswith("\n"))
        self.assertEqual(
            json.loads(out.getvalue()),
            {
                "groups": [
                    {"key": "a", "count": 1, "total": "1.00"},
                    {"key": "b", "count": 1, "total": "1.00"},
                    {"key": "c", "count": 1, "total": "-0.50"},
                ],
                "count": 3,
                "grand_total": "1.50",
            },
        )

    def test_empty_file_json(self):
        path = self.write("region,product,amount\n")
        out = io.StringIO()
        self.assertEqual(main([path, "--format", "json"], stdout=out), 0)
        self.assertEqual(json.loads(out.getvalue()), {"groups": [], "count": 0, "grand_total": "0.00"})

    def test_explicit_text_is_default(self):
        path = self.write("region,product,amount\nn,p,1\n")
        a, b = io.StringIO(), io.StringIO()
        main([path], stdout=a)
        main([path, "--format", "text"], stdout=b)
        self.assertEqual(a.getvalue(), b.getvalue())

    def test_unknown_format_is_a_usage_error(self):
        path = self.write("region,product,amount\nn,p,1\n")
        with contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as ctx:
                main([path, "--format", "xml"], stdout=io.StringIO())
        self.assertEqual(ctx.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
