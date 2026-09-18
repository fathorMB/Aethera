"""Bucket order totals into fixed-width bands, for a quick distribution
report ("how many orders are between 0 and 10 EUR, 10 and 20, ...").
"""

from __future__ import annotations

from typing import Dict, List, Tuple

from ..stages.enrich_currency import EnrichedOrder


def histogram(enriched_orders: List[EnrichedOrder], bucket_width_cents: int) -> Dict[Tuple[int, int], int]:
    """`{(low_cents, high_cents): count}` for every non-empty bucket, where
    `low_cents` is inclusive and `high_cents` exclusive. Buckets with no
    orders are omitted rather than listed with a zero count. Negative
    totals fall into the bucket below zero, `(-bucket_width_cents, 0)`.

    Raises `ValueError` if `bucket_width_cents` is not positive.
    """
    if bucket_width_cents <= 0:
        raise ValueError(f"bucket_width_cents must be positive, got {bucket_width_cents}")
    counts: Dict[Tuple[int, int], int] = {}
    for eo in enriched_orders:
        total = eo.total_target_cents
        index = total // bucket_width_cents
        low = index * bucket_width_cents
        high = low + bucket_width_cents
        counts[(low, high)] = counts.get((low, high), 0) + 1
    return counts
