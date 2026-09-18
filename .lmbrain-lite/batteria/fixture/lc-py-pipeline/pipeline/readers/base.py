"""Shared helpers for the file readers.

Every reader in this package needs to turn a raw string field into an int
and complain with a `ReaderError` that names the file and line, instead of
letting a bare `ValueError` escape with no context.
"""

from __future__ import annotations

from ..errors import ReaderError


def parse_int_field(raw: str, *, path: str, line_no: int, field: str) -> int:
    try:
        return int(raw)
    except (TypeError, ValueError):
        raise ReaderError(path, line_no, f"{field!r} is not an integer: {raw!r}") from None


def require_field(row: dict, field: str, *, path: str, line_no: int) -> str:
    value = row.get(field)
    if value is None or value == "":
        raise ReaderError(path, line_no, f"missing field {field!r}")
    return value
