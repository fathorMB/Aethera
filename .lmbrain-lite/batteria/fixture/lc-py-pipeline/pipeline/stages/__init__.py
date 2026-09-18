"""Record -> record transforms that run over a whole batch.

Every stage returns a new list; none of them mutate their input in place,
so a caller can always compare before and after.
"""

from .apply_discount import apply_bulk_discount
from .dedupe import dedupe_orders
from .enrich_currency import EnrichedOrder, enrich_orders
from .enrich_geo import REGION_BY_COUNTRY, region_for_country
from .filter_amount import filter_min_amount
from .filter_date_range import filter_date_range
from .normalize import normalize_customers, normalize_orders
from .sort_stage import sort_orders_by_date

__all__ = [
    "apply_bulk_discount",
    "dedupe_orders",
    "EnrichedOrder",
    "enrich_orders",
    "REGION_BY_COUNTRY",
    "region_for_country",
    "filter_min_amount",
    "filter_date_range",
    "normalize_customers",
    "normalize_orders",
    "sort_orders_by_date",
]
