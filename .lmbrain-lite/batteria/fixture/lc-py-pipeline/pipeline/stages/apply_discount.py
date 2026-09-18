"""An optional bulk-discount stage.

Not wired into `pipeline.orchestrator.run`: this is a stage a caller opts
into (a promotional report, a "what-if" simulation) by running it before
`enrich_currency`. It never mutates the input orders; it returns new ones.
"""

from __future__ import annotations

from dataclasses import replace
from typing import List, Tuple

from ..domain.order import Order

# (minimum quantity, discount in percent) pairs, checked from the highest
# threshold down, so an order gets the best tier it qualifies for.
DEFAULT_TIERS: Tuple[Tuple[int, int], ...] = ((50, 15), (20, 10), (10, 5))


def _discount_percent_for(quantity: int, tiers: Tuple[Tuple[int, int], ...]) -> int:
    for threshold, percent in tiers:
        if quantity >= threshold:
            return percent
    return 0


def apply_bulk_discount(orders: List[Order], tiers: Tuple[Tuple[int, int], ...] = DEFAULT_TIERS) -> List[Order]:
    """Return new orders with `unit_price_cents` reduced by the discount
    tier their `quantity` qualifies for. Rounds each unit price down to
    the nearest cent, so the discount never rounds in the customer's
    favor beyond what the percentage promises.
    """
    discounted: List[Order] = []
    for order in orders:
        percent = _discount_percent_for(order.quantity, tiers)
        if percent == 0:
            discounted.append(order)
            continue
        new_price = (order.unit_price_cents * (100 - percent)) // 100
        discounted.append(replace(order, unit_price_cents=new_price))
    return discounted
