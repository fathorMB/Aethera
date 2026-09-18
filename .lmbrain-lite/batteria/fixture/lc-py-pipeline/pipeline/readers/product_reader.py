"""Read the product catalog from a CSV file.

Expected header: `product_id,name,category,unit_price_cents`. Like
`read_orders_csv`, extra columns are ignored and a missing required column
raises `ReaderError` naming the row.
"""

from __future__ import annotations

import csv
from pathlib import Path
from typing import List

from ..domain.product import Product
from .base import parse_int_field, require_field

REQUIRED_FIELDS = ("product_id", "name", "category", "unit_price_cents")


def read_products_csv(path: str | Path) -> List[Product]:
    products: List[Product] = []
    with open(path, newline="", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        for line_no, row in enumerate(reader, start=2):
            for field in REQUIRED_FIELDS:
                require_field(row, field, path=str(path), line_no=line_no)
            products.append(
                Product(
                    product_id=row["product_id"],
                    name=row["name"],
                    category=row["category"],
                    unit_price_cents=parse_int_field(
                        row["unit_price_cents"], path=str(path), line_no=line_no, field="unit_price_cents"
                    ),
                )
            )
    return products
