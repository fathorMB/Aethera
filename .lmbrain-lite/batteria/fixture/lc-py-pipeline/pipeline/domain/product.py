"""The product record."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Product:
    """A catalog entry.

    `unit_price_cents` is the catalog price and is informational only: an
    order carries its own `unit_price_cents`, which may differ (discounts,
    historical price at the time of the order). `category` groups products
    for reporting and is free text.
    """

    product_id: str
    name: str
    category: str
    unit_price_cents: int
