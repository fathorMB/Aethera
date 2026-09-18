"""Read refunds from a CSV file.

Expected header: `refund_id,order_id,amount_cents,reason,refund_date`.
`reason` may be blank (an export that has not been annotated yet); every
other column is required.
"""

from __future__ import annotations

import csv
from pathlib import Path
from typing import List

from ..domain.refund import Refund
from .base import parse_int_field, require_field

REQUIRED_FIELDS = ("refund_id", "order_id", "amount_cents", "refund_date")


def read_refunds_csv(path: str | Path) -> List[Refund]:
    refunds: List[Refund] = []
    with open(path, newline="", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        for line_no, row in enumerate(reader, start=2):
            for field in REQUIRED_FIELDS:
                require_field(row, field, path=str(path), line_no=line_no)
            refunds.append(
                Refund(
                    refund_id=row["refund_id"],
                    order_id=row["order_id"],
                    amount_cents=parse_int_field(
                        row["amount_cents"], path=str(path), line_no=line_no, field="amount_cents"
                    ),
                    reason=row.get("reason") or "",
                    refund_date=row["refund_date"],
                )
            )
    return refunds
