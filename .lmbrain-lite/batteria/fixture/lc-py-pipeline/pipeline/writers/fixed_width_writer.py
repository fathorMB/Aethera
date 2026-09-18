"""Write a ranked totals table as fixed-width text, for systems that still
expect a column-aligned plain-text report instead of CSV or JSON.
"""

from __future__ import annotations

from typing import List, Tuple

from ..util.numbers import format_cents

NAME_WIDTH = 24
TOTAL_WIDTH = 12


def render_top_customers_fixed_width(rows: List[Tuple[str, int]]) -> str:
    """`rows` is `(name, total_cents)`, already ranked. Each line is the
    name left-padded to `NAME_WIDTH` columns (truncated with no ellipsis if
    longer) followed by the amount right-aligned in `TOTAL_WIDTH` columns.
    Ends with a trailing newline, including when `rows` is empty (which
    then renders as an empty string).
    """
    lines = []
    for name, total_cents in rows:
        label = name[:NAME_WIDTH].ljust(NAME_WIDTH)
        amount = format_cents(total_cents).rjust(TOTAL_WIDTH)
        lines.append(f"{label}{amount}")
    return "".join(line + "\n" for line in lines)
