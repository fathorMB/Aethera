"""Read orders from a CSV file.

Expected header: `order_id,customer_id,product_id,quantity,unit_price_cents,
currency,order_date`. Column order does not matter, extra columns are
ignored, and the header must be present (this is `csv.DictReader`'s default
behaviour).
"""

from __future__ import annotations

import csv
from pathlib import Path
from typing import List

from ..domain.order import Order
from .base import parse_int_field, require_field

REQUIRED_FIELDS = (
    "order_id",
    "customer_id",
    "product_id",
    "quantity",
    "unit_price_cents",
    "currency",
    "order_date",
)


def read_orders_csv(path: str | Path) -> List[Order]:
    """Read every row of `path` into an `Order`. Raises `ReaderError` on a
    malformed row; a well-formed but semantically invalid row (negative
    quantity, unknown currency, ...) is returned as-is, that is the job of
    `pipeline.validators.order_rules`.
    """
    orders: List[Order] = []
    with open(path, newline="", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        for line_no, row in enumerate(reader, start=2):  # header is line 1
            for field in REQUIRED_FIELDS:
                require_field(row, field, path=str(path), line_no=line_no)
            orders.append(
                Order(
                    order_id=row["order_id"],
                    customer_id=row["customer_id"],
                    product_id=row["product_id"],
                    quantity=parse_int_field(
                        row["quantity"], path=str(path), line_no=line_no, field="quantity"
                    ),
                    unit_price_cents=parse_int_field(
                        row["unit_price_cents"], path=str(path), line_no=line_no, field="unit_price_cents"
                    ),
                    currency=row["currency"],
                    order_date=row["order_date"],
                )
            )
    return orders
