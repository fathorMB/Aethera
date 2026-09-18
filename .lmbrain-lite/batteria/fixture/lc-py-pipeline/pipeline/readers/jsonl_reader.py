"""Read customers from a JSON-lines file: one JSON object per line."""

from __future__ import annotations

import json
from pathlib import Path
from typing import List

from ..domain.customer import Customer
from ..errors import ReaderError
from .base import require_field


def read_customers_jsonl(path: str | Path) -> List[Customer]:
    """Read every non-blank line of `path` into a `Customer`.

    Blank lines are skipped so a trailing newline in the file does not
    raise. A line that is not valid JSON, or is missing a required key,
    raises `ReaderError` naming the line number (1-based).
    """
    customers: List[Customer] = []
    with open(path, encoding="utf-8") as fh:
        for line_no, line in enumerate(fh, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise ReaderError(str(path), line_no, f"invalid JSON: {exc}") from None
            for field in ("customer_id", "name", "country"):
                require_field(row, field, path=str(path), line_no=line_no)
            customers.append(
                Customer(
                    customer_id=row["customer_id"],
                    name=row["name"],
                    country=row["country"],
                    segment=row.get("segment", "retail"),
                )
            )
    return customers
