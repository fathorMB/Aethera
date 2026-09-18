"""A thin wrapper over `pipeline.aggregators.category_totals`, ranked the
same way the top-customers report is (see `pipeline.util.ordering`), so
the two reports read consistently: highest spend first, ties broken by
name.
"""

from __future__ import annotations

from typing import List, Tuple

from ..domain.product import Product
from ..stages.enrich_currency import EnrichedOrder
from ..aggregators.category_totals import totals_by_category
from ..util.ordering import rank_totals


def category_breakdown(enriched_orders: List[EnrichedOrder], products: List[Product]) -> List[Tuple[str, int]]:
    totals = totals_by_category(enriched_orders, products)
    return rank_totals(totals)
