"""Formatting for integer-cents amounts.

Every amount in this pipeline is an integer number of cents; nothing does
money arithmetic in floating point. These helpers are the only place cents
turn into the decimal string a report shows.
"""

from __future__ import annotations


def format_cents(cents: int) -> str:
    """`1234 -> "12.34"`, `-5 -> "-0.05"`, `0 -> "0.00"`."""
    sign = "-" if cents < 0 else ""
    cents = abs(cents)
    whole, frac = divmod(cents, 100)
    return f"{sign}{whole}.{frac:02d}"


def parse_cents(text: str) -> int:
    """The inverse of `format_cents`, for decimal strings like `"12.3"` or
    `"-0.05"`. Raises `ValueError` on anything else, including more than
    two fraction digits (which would silently lose precision).
    """
    text = text.strip()
    negative = text.startswith("-")
    if negative:
        text = text[1:]
    if "." not in text:
        whole_part, frac_part = text, "0"
    else:
        whole_part, frac_part = text.split(".", 1)
    if len(frac_part) > 2:
        raise ValueError(f"more than two decimal digits: {text!r}")
    frac_part = frac_part.ljust(2, "0")
    if not whole_part.isdigit() or not frac_part.isdigit():
        raise ValueError(f"not a decimal amount: {text!r}")
    cents = int(whole_part) * 100 + int(frac_part)
    return -cents if negative else cents
