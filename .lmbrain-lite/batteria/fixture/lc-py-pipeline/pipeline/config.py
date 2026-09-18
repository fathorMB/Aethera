"""Run-time configuration for `pipeline.orchestrator.run`.

Kept as one small dataclass rather than scattered keyword arguments so the
orchestrator's signature stays stable while new knobs are added here.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional, Tuple


@dataclass(frozen=True)
class PipelineConfig:
    """Options for a single pipeline run.

    `target_currency`: every order total is converted into this currency
        before customers are ranked, so totals in different currencies can
        be compared. Must be a code known to `pipeline.util.currency`.
    `top_n`: how many customers appear in the ranked report. Zero means an
        empty ranking; there is no upper bound.
    `max_order_quantity`: orders with a larger quantity are rejected by
        `pipeline.validators.order_rules` as data-entry mistakes.
    `date_range`: if set, a `(start, end)` pair of ISO dates; orders outside
        the range (inclusive on both ends) are dropped before aggregation.
    """

    target_currency: str = "EUR"
    top_n: int = 5
    max_order_quantity: int = 1000
    date_range: Optional[Tuple[str, str]] = None


DEFAULT_CONFIG = PipelineConfig()
