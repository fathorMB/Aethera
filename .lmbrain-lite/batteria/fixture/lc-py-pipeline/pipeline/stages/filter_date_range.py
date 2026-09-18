"""Drop orders outside an inclusive date range.

Kept as its own stage (rather than inlined in the orchestrator) so it can
be unit-tested on its own and reused by a future report that needs a
different range than the main sales report.
"""

from __future__ import annotations

from typing import List, Optional, Tuple

from ..domain.order import Order
from ..util.dates import in_range


def filter_date_range(orders: List[Order], date_range: Optional[Tuple[str, str]]) -> List[Order]:
    """Keep only orders whose `order_date` falls within `date_range`
    (inclusive on both ends). `date_range=None` is a no-op: every order is
    kept, in the same order they were given.
    """
    if date_range is None:
        return list(orders)
    start, end = date_range
    return [order for order in orders if in_range(order.order_date, start, end)]
