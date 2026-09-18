"""Turn a summary into text: CSV, JSON, fixed-width, or Markdown."""

from .csv_writer import write_totals_csv
from .fixed_width_writer import render_top_customers_fixed_width
from .json_writer import render_top_customers_json
from .markdown_writer import render_top_customers_markdown

__all__ = [
    "write_totals_csv",
    "render_top_customers_fixed_width",
    "render_top_customers_json",
    "render_top_customers_markdown",
]
