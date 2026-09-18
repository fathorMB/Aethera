"""Drop orders whose gross amount (in their own currency) is below a floor.

Not part of the default `pipeline.orchestrator.run` pipeline: this is an
optional stage a caller can splice in (for example, a report that only
cares about orders worth reviewing) via `pipeline.stages.base.apply_stages`.
"""

from __future__ import annotations

from typing import List

from ..domain.order import Order


def filter_min_amount(orders: List[Order], min_gross_cents: int) -> List[Order]:
    """Keep orders whose `gross_cents` is at least `min_gross_cents`.

    `min_gross_cents <= 0` keeps every order, including ones with a gross
    of exactly zero (which validation would already have rejected for a
    non-positive quantity or price, but this stage does not assume that).
    """
    return [order for order in orders if order.gross_cents >= min_gross_cents]
