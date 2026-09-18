"""Group order totals by product category.

Unlike `group_by.totals_by_customer`, this needs the product catalog to
resolve `product_id -> category`, so it takes that mapping explicitly
rather than importing a reader.
"""

from __future__ import annotations

from typing import Dict, List

from ..domain.product import Product
from ..stages.enrich_currency import EnrichedOrder

UNKNOWN_CATEGORY = "uncategorized"


def totals_by_category(enriched_orders: List[EnrichedOrder], products: List[Product]) -> Dict[str, int]:
    """`{category: sum of total_target_cents}`.

    An order referencing a `product_id` that is not in `products` is
    grouped under `UNKNOWN_CATEGORY` rather than dropped or raising: a
    stale catalog should degrade the report, not break it.
    """
    category_by_product = {p.product_id: p.category for p in products}
    totals: Dict[str, int] = {}
    for eo in enriched_orders:
        category = category_by_product.get(eo.order.product_id, UNKNOWN_CATEGORY)
        totals[category] = totals.get(category, 0) + eo.total_target_cents
    return totals
