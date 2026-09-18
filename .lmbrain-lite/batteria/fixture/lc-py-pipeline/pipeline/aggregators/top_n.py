"""Rank customers by total spend and keep the top `n`."""

from __future__ import annotations

from typing import Dict, List, Tuple

from ..util.ordering import rank_totals


def top_n_by_total(totals: Dict[str, int], n: int) -> List[Tuple[str, int]]:
    """The `n` customers with the highest total, highest first.

    Ranking (including how ties are broken) is entirely delegated to
    `pipeline.util.ordering.rank_totals`; this function only truncates.
    `n <= 0` returns an empty list, `n` larger than the number of entries
    returns all of them.
    """
    return rank_totals(totals)[: max(n, 0)]
