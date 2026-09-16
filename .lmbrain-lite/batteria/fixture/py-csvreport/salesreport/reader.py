"""Read sales rows from CSV text."""

import csv
import io
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation


@dataclass(frozen=True)
class Row:
    region: str
    product: str
    amount: Decimal


class BadRow(ValueError):
    pass


def read_rows(text):
    """Parse CSV with a header line `region,product,amount`. Blank lines are skipped."""
    rows = []
    reader = csv.DictReader(io.StringIO(text))
    for line_no, rec in enumerate(reader, start=2):
        if not any((v or "").strip() for v in rec.values()):
            continue
        try:
            amount = Decimal(rec["amount"].strip())
        except (InvalidOperation, AttributeError) as exc:
            raise BadRow(f"line {line_no}: bad amount {rec.get('amount')!r}") from exc
        rows.append(Row(rec["region"].strip(), rec["product"].strip(), amount))
    return rows
