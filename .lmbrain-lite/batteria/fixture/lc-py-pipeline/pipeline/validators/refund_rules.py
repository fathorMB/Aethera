"""Validation rules for `pipeline.domain.refund.Refund`."""

from __future__ import annotations

from ..domain.refund import Refund
from .base import ValidationResult, run_rules


def _rule_id_present(r: Refund) -> str | None:
    if not r.refund_id.strip():
        return "refund_id is blank"
    return None


def _rule_order_id_present(r: Refund) -> str | None:
    if not r.order_id.strip():
        return "order_id is blank"
    return None


def _rule_positive_amount(r: Refund) -> str | None:
    if r.amount_cents <= 0:
        return f"amount_cents must be positive, got {r.amount_cents}"
    return None


RULES = [_rule_id_present, _rule_order_id_present, _rule_positive_amount]


def validate_refund(refund: Refund) -> ValidationResult:
    return run_rules(refund, RULES)
