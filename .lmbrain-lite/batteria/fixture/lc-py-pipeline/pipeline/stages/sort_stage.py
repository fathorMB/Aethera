"""Put orders back into chronological order.

Readers make no promise about row order (a CSV export can be sorted by
customer, by product, or not sorted at all), so the orchestrator sorts by
date once, right after the date-range filter, before anything downstream
depends on order.
"""

from __future__ import annotations

from typing import List

from ..domain.order import Order


def sort_orders_by_date(orders: List[Order]) -> List[Order]:
    """Sort by `order_date` ascending (oldest first).

    `order_date` is an ISO-8601 string, so a plain lexicographic sort is
    already a correct chronological sort — no date parsing needed. Orders
    sharing the same date keep their relative order (Python's sort is
    stable).
    """
    return sorted(orders, key=lambda order: order.order_date)
