"""Group enriched orders by customer and sum their converted totals."""

from __future__ import annotations

from typing import Dict, List

from ..stages.enrich_currency import EnrichedOrder


def totals_by_customer(enriched_orders: List[EnrichedOrder]) -> Dict[str, int]:
    """`{customer_id: sum of total_target_cents}` for every customer that
    placed at least one order in `enriched_orders`. A customer with no
    orders simply does not appear; callers that need every known customer
    represented should default-fill with zero themselves.
    """
    totals: Dict[str, int] = {}
    for eo in enriched_orders:
        cid = eo.order.customer_id
        totals[cid] = totals.get(cid, 0) + eo.total_target_cents
    return totals
