"""Validation rules for `pipeline.domain.order.Order`.

`validate_order` takes the set of known currency codes and the configured
maximum quantity as parameters rather than importing them, so the rules can
be unit-tested without going through `pipeline.util.currency` or
`pipeline.config`.
"""

from __future__ import annotations

from typing import Iterable

from ..domain.order import Order
from .base import ValidationResult, run_rules


def _rule_positive_quantity(order: Order) -> str | None:
    if order.quantity <= 0:
        return f"quantity must be positive, got {order.quantity}"
    return None


def _make_rule_max_quantity(max_quantity: int):
    def _rule_max_quantity(order: Order) -> str | None:
        # `max_quantity` itself is allowed; only strictly more is a mistake.
        # (A quick read might expect `>=` here — it would reject the exact
        # boundary the config promises is still valid.)
        if order.quantity > max_quantity:
            return f"quantity {order.quantity} exceeds the maximum of {max_quantity}"
        return None

    return _rule_max_quantity


def _rule_positive_price(order: Order) -> str | None:
    if order.unit_price_cents <= 0:
        return f"unit_price_cents must be positive, got {order.unit_price_cents}"
    return None


def _make_rule_known_currency(known_currencies: Iterable[str]):
    known = set(known_currencies)

    def _rule_known_currency(order: Order) -> str | None:
        if order.currency not in known:
            return f"unknown currency: {order.currency!r}"
        return None

    return _rule_known_currency


def validate_order(order: Order, *, known_currencies: Iterable[str], max_quantity: int = 1000) -> ValidationResult:
    rules = [
        _rule_positive_quantity,
        _make_rule_max_quantity(max_quantity),
        _rule_positive_price,
        _make_rule_known_currency(known_currencies),
    ]
    return run_rules(order, rules)
