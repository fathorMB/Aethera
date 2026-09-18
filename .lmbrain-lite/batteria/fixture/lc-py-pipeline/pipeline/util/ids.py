"""A small sanity check for the free-form id fields (order_id, customer_id,
product_id, refund_id).

The pipeline does not enforce a specific id format end to end — different
upstream systems mint ids differently — but every id must at least be
non-blank and free of embedded whitespace, since ids are used as dictionary
keys and printed in reports.
"""

from __future__ import annotations


def is_valid_id(value: str) -> bool:
    stripped = value.strip()
    return bool(stripped) and stripped == value and " " not in stripped
