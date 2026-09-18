"""Small string helpers."""

from __future__ import annotations


def strip_or_empty(value: str | None) -> str:
    """`value.strip()`, or `""` if `value` is `None`."""
    return value.strip() if value is not None else ""


def normalize_whitespace(value: str) -> str:
    """Collapse any run of whitespace in `value` into a single space, and
    strip the ends. Used when rendering free-text fields (a customer's
    segment, a refund reason) so a stray tab or double space from a CSV
    export does not leak into a report.
    """
    return " ".join(value.split())


def truncate(value: str, max_len: int, *, suffix: str = "...") -> str:
    """Shorten `value` to at most `max_len` characters, appending `suffix`
    when it was actually shortened. `max_len` counts the suffix, so the
    result is never longer than `max_len`.
    """
    if len(value) <= max_len:
        return value
    if max_len <= len(suffix):
        return suffix[:max_len]
    return value[: max_len - len(suffix)] + suffix
