"""Validation rules for `pipeline.domain.product.Product`."""

from __future__ import annotations

from ..domain.product import Product
from .base import ValidationResult, run_rules


def _rule_id_present(p: Product) -> str | None:
    if not p.product_id.strip():
        return "product_id is blank"
    return None


def _rule_category_present(p: Product) -> str | None:
    if not p.category.strip():
        return "category is blank"
    return None


def _rule_non_negative_price(p: Product) -> str | None:
    # Zero is allowed on purpose: some catalogs list free promotional
    # items at a listed price of zero. Only a negative price is a mistake.
    if p.unit_price_cents < 0:
        return f"unit_price_cents cannot be negative, got {p.unit_price_cents}"
    return None


RULES = [_rule_id_present, _rule_category_present, _rule_non_negative_price]


def validate_product(product: Product) -> ValidationResult:
    return run_rules(product, RULES)
