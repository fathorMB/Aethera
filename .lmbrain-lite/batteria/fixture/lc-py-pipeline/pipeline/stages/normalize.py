"""Normalize orders and customers before validation.

Normalizing before validating means a customer whose country arrived as
`"us"` is not rejected by `pipeline.validators.customer_rules` for failing
to look like `"US"`: normalization runs first, so validators only ever see
canonical values.
"""

from __future__ import annotations

from dataclasses import replace
from typing import List

from ..domain.customer import Customer
from ..domain.order import Order
from ..util.text import strip_or_empty


def normalize_orders(orders: List[Order]) -> List[Order]:
    """Trim whitespace from the id fields and uppercase the currency code.

    `order_date` and `unit_price_cents` are left untouched: they are either
    already canonical (numbers) or the reader's job (dates).
    """
    return [
        replace(
            order,
            order_id=strip_or_empty(order.order_id),
            customer_id=strip_or_empty(order.customer_id),
            product_id=strip_or_empty(order.product_id),
            currency=order.currency.strip().upper(),
        )
        for order in orders
    ]


def normalize_customers(customers: List[Customer]) -> List[Customer]:
    """Trim whitespace from `name` and uppercase `country`."""
    return [
        Customer(
            customer_id=strip_or_empty(c.customer_id),
            name=strip_or_empty(c.name),
            country=c.country.strip().upper(),
            segment=c.segment,
        )
        for c in customers
    ]
