"""Attach each order's total, converted into the report's target currency."""

from __future__ import annotations

from dataclasses import dataclass
from typing import List

from ..domain.order import Order
from ..util.currency import convert


@dataclass(frozen=True)
class EnrichedOrder:
    """An order plus its gross total converted into `target_currency`."""

    order: Order
    total_target_cents: int


def enrich_orders(orders: List[Order], target_currency: str) -> List[EnrichedOrder]:
    return [
        EnrichedOrder(
            order=order,
            total_target_cents=convert(order.gross_cents, order.currency, target_currency),
        )
        for order in orders
    ]
