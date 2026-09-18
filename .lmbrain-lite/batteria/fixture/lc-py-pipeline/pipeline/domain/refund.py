"""The refund record.

Refunds are read and validated by this pipeline but are not yet folded into
the sales report; `pipeline.orchestrator.run` does not take them as an
argument at all. Kept here, and validated, so the schema is in one place
when a refunds report is written.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Refund:
    refund_id: str
    order_id: str
    amount_cents: int
    reason: str
    refund_date: str
