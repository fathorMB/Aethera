"""Cross-record checks: does an order actually point at a customer and a
product that exist?

These are kept separate from `order_rules`, which only ever looks at one
order at a time: a referential check needs the whole batch of customers
and products to answer "does this id exist", so it runs as its own pass
over already-validated, already-deduped orders.
"""

from __future__ import annotations

from typing import Iterable, List

from ..domain.order import Order


def orders_with_unknown_customer(orders: Iterable[Order], known_customer_ids: Iterable[str]) -> List[str]:
    """The `order_id` of every order whose `customer_id` is not in
    `known_customer_ids`, in the order the orders were given.
    """
    known = set(known_customer_ids)
    return [order.order_id for order in orders if order.customer_id not in known]


def orders_with_unknown_product(orders: Iterable[Order], known_product_ids: Iterable[str]) -> List[str]:
    known = set(known_product_ids)
    return [order.order_id for order in orders if order.product_id not in known]
