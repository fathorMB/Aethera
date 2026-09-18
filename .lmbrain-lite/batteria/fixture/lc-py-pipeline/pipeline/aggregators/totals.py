"""Whole-batch totals that do not need grouping."""

from __future__ import annotations

from typing import List

from ..stages.enrich_currency import EnrichedOrder


def grand_total(enriched_orders: List[EnrichedOrder]) -> int:
    """The sum of every order's converted total, in target-currency cents."""
    return sum(eo.total_target_cents for eo in enriched_orders)
