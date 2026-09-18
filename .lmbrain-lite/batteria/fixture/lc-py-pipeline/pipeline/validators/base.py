"""The shared result type and runner used by every rule module."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable, List, TypeVar

T = TypeVar("T")
Rule = Callable[[T], "str | None"]


@dataclass
class ValidationResult:
    """The outcome of running every rule against one record.

    `errors` is empty when `valid` is True. A rule signals a failure by
    returning a human-readable reason string; returning `None` means the
    rule passed.
    """

    valid: bool
    errors: List[str] = field(default_factory=list)


def run_rules(record: T, rules: List[Rule]) -> ValidationResult:
    """Run every rule against `record` and collect every failure.

    Unlike a chain that stops at the first failure, this always runs all
    the rules, so a rejected record's error list explains everything that
    was wrong with it, not just the first thing found.
    """
    errors = [msg for rule in rules if (msg := rule(record)) is not None]
    return ValidationResult(valid=not errors, errors=errors)
