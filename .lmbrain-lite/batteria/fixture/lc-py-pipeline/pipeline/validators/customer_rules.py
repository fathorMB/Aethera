"""Validation rules for `pipeline.domain.customer.Customer`."""

from __future__ import annotations

from ..domain.customer import Customer
from .base import ValidationResult, run_rules


def _rule_id_present(c: Customer) -> str | None:
    if not c.customer_id.strip():
        return "customer_id is blank"
    return None


def _rule_name_present(c: Customer) -> str | None:
    if not c.name.strip():
        return "name is blank"
    return None


def _rule_country_code_length(c: Customer) -> str | None:
    # ISO 3166-1 alpha-2: exactly two letters. A three-letter or numeric
    # code is a data-entry mistake upstream and is rejected here rather
    # than silently truncated or looked up.
    if len(c.country) != 2 or not c.country.isalpha():
        return f"country is not a two-letter code: {c.country!r}"
    return None


RULES = [_rule_id_present, _rule_name_present, _rule_country_code_length]


def validate_customer(customer: Customer) -> ValidationResult:
    return run_rules(customer, RULES)
