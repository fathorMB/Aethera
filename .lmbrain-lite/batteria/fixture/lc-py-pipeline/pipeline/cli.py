"""Command-line entry point: `python -m pipeline.cli ORDERS.csv CUSTOMERS.jsonl`.

Prints the top-customers ranking as a Markdown table on stdout. Not
exercised by the test suite beyond being importable; it exists so the
package is runnable end to end, not only through its library API.
"""

from __future__ import annotations

import argparse
import sys
from typing import List, Optional

from .config import PipelineConfig
from .orchestrator import run
from .readers.csv_reader import read_orders_csv
from .readers.jsonl_reader import read_customers_jsonl
from .writers.markdown_writer import render_top_customers_markdown


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="pipeline", description="Build a sales report from an order feed.")
    parser.add_argument("orders_csv", help="path to the orders CSV file")
    parser.add_argument("customers_jsonl", help="path to the customers JSON-lines file")
    parser.add_argument("--currency", default="EUR", help="target currency for the report (default: EUR)")
    parser.add_argument("--top", type=int, default=5, help="how many customers to rank (default: 5)")
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    args = build_parser().parse_args(argv)
    orders = read_orders_csv(args.orders_csv)
    customers = read_customers_jsonl(args.customers_jsonl)
    config = PipelineConfig(target_currency=args.currency, top_n=args.top)
    report = run(orders, customers, config)
    rows = [(name, total) for _, name, total in report.top_customers]
    sys.stdout.write(render_top_customers_markdown(rows, config.target_currency))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
