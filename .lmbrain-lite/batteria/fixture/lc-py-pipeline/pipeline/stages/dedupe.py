"""Drop duplicate orders.

A duplicate `order_id` can happen when an upstream export retries and
appends instead of overwriting. The rule is: keep the first occurrence,
drop everything after it, and do not reorder the survivors.
"""

from __future__ import annotations

from typing import List

from ..domain.order import Order


def dedupe_orders(orders: List[Order]) -> List[Order]:
    seen: set[str] = set()
    kept: List[Order] = []
    for order in orders:
        if order.order_id in seen:
            continue
        seen.add(order.order_id)
        kept.append(order)
    return kept
