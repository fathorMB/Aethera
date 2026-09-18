"""Total spend by customer region, for a geography breakdown report."""

from __future__ import annotations

from typing import Dict, List

from ..domain.customer import Customer
from ..stages.enrich_currency import EnrichedOrder
from ..stages.enrich_geo import region_for_country
from ..util.collections_util import index_by


def region_breakdown(enriched_orders: List[EnrichedOrder], customers: List[Customer]) -> Dict[str, int]:
    """`{region: sum of total_target_cents}`, grouping by
    `pipeline.stages.enrich_geo.region_for_country` on each order's
    customer's country. An order whose customer is not in `customers` is
    grouped under `"Other"`, the same fallback `region_for_country` uses
    for an unrecognized country code.
    """
    customers_by_id = index_by(customers, lambda c: c.customer_id)
    totals: Dict[str, int] = {}
    for eo in enriched_orders:
        customer = customers_by_id.get(eo.order.customer_id)
        region = region_for_country(customer.country) if customer is not None else "Other"
        totals[region] = totals.get(region, 0) + eo.total_target_cents
    return totals
