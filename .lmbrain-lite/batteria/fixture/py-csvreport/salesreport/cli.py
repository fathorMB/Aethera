"""Entry point: python -m salesreport.cli FILE [--by region|product]"""

import argparse
import sys

from .aggregate import group_rows
from .reader import BadRow, read_rows
from .render import render_text


def build_parser():
    p = argparse.ArgumentParser(prog="salesreport")
    p.add_argument("file", help="CSV file with region,product,amount")
    p.add_argument("--by", choices=["region", "product"], default="region")
    return p


def main(argv=None, stdout=None):
    stdout = stdout or sys.stdout
    args = build_parser().parse_args(argv)
    with open(args.file, encoding="utf-8") as fh:
        text = fh.read()
    try:
        rows = read_rows(text)
    except BadRow as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    table = group_rows(rows, args.by)
    stdout.write(render_text(table))
    return 0


if __name__ == "__main__":
    sys.exit(main())
