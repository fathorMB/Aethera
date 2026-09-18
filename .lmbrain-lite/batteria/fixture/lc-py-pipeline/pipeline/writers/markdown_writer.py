"""Render the top-customers ranking as a Markdown table."""

from __future__ import annotations

from typing import List, Tuple

from ..util.numbers import format_cents


def render_top_customers_markdown(rows: List[Tuple[str, int]], currency: str) -> str:
    """`rows` is `(name, total_cents)`, already ranked in the order they
    should be printed (rank 1 first). Returns the table as a single string
    ending in a newline; an empty `rows` still returns a header-only table.
    """
    lines = ["| # | customer | total |", "| -: | :- | -: |"]
    for rank, (name, total_cents) in enumerate(rows, start=1):
        lines.append(f"| {rank} | {name} | {format_cents(total_cents)} {currency} |")
    return "\n".join(lines) + "\n"
