"""Wire every stage together into one `run` call.

This module owns the *order* the stages run in (see README.md); it
contains no business rules of its own beyond that ordering.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List, Tuple

from .aggregators.group_by import totals_by_customer
from .aggregators.top_n import top_n_by_total
from .aggregators.totals import grand_total
from .config import DEFAULT_CONFIG, PipelineConfig
from .domain.customer import Customer
from .domain.order import Order
from .stages.dedupe import dedupe_orders
from .stages.enrich_currency import enrich_orders
from .stages.filter_date_range import filter_date_range
from .stages.normalize import normalize_customers, normalize_orders
from .stages.sort_stage import sort_orders_by_date
from .util.currency import known_currencies
from .validators.order_rules import validate_order


@dataclass
class Report:
    """The result of one `run`.

    `top_customers` is `(customer_id, name, total_cents)`, already ranked
    highest spend first, at most `config.top_n` entries long. `name` falls
    back to the customer id itself when an order references a customer
    that was not in the customer feed.
    """

    total_orders: int
    accepted_orders: int
    rejected_order_ids: List[str] = field(default_factory=list)
    grand_total_cents: int = 0
    top_customers: List[Tuple[str, str, int]] = field(default_factory=list)


def run(orders: List[Order], customers: List[Customer], config: PipelineConfig = DEFAULT_CONFIG) -> Report:
    """Build a `Report` from raw orders and customers.

    Steps, in order: normalize, validate (rejects are recorded, not
    fatal), dedupe, optional date-range filter, sort by date, convert
    currencies, group by customer, rank. See README.md for why this order
    matters.
    """
    norm_customers = normalize_customers(customers)
    customers_by_id = {c.customer_id: c for c in norm_customers}

    norm_orders = normalize_orders(orders)
    currencies = known_currencies()

    accepted: List[Order] = []
    rejected: List[str] = []
    for order in norm_orders:
        result = validate_order(order, known_currencies=currencies, max_quantity=config.max_order_quantity)
        if result.valid:
            accepted.append(order)
        else:
            rejected.append(order.order_id)

    accepted = dedupe_orders(accepted)
    accepted = filter_date_range(accepted, config.date_range)
    accepted = sort_orders_by_date(accepted)

    enriched = enrich_orders(accepted, config.target_currency)
    totals: Dict[str, int] = totals_by_customer(enriched)
    ranked = top_n_by_total(totals, config.top_n)

    top_customers = [
        (customer_id, customers_by_id[customer_id].name if customer_id in customers_by_id else customer_id, total)
        for customer_id, total in ranked
    ]

    return Report(
        total_orders=len(norm_orders),
        accepted_orders=len(accepted),
        rejected_order_ids=rejected,
        grand_total_cents=grand_total(enriched),
        top_customers=top_customers,
    )
