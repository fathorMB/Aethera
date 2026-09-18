"""Write a ranked totals table as CSV."""

from __future__ import annotations

import csv
from pathlib import Path
from typing import List, Tuple

from ..util.numbers import format_cents


def write_totals_csv(path: str | Path, rows: List[Tuple[str, str, int]]) -> None:
    """Write `(customer_id, name, total_cents)` rows to `path` as CSV with
    a header, formatting the total the way a person would type it
    (`"12.34"`, always two decimals).
    """
    with open(path, "w", newline="", encoding="utf-8") as fh:
        writer = csv.writer(fh)
        writer.writerow(["customer_id", "name", "total"])
        for customer_id, name, total_cents in rows:
            writer.writerow([customer_id, name, format_cents(total_cents)])
