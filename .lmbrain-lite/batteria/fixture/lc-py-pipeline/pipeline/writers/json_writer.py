"""Write a ranked totals table as a single JSON document.

Amounts are written as strings with two decimals (like the CSV and
Markdown writers), not as floats: a float would silently misrepresent an
exact cents value for some amounts, and downstream consumers expect the
same decimal-string convention everywhere in this pipeline's output.
"""

from __future__ import annotations

import json
from typing import List, Tuple

from ..util.numbers import format_cents


def render_top_customers_json(rows: List[Tuple[str, str, int]], currency: str) -> str:
    """`rows` is `(customer_id, name, total_cents)`, already ranked. Returns
    a JSON string (no trailing newline) shaped like:
    `{"currency": "EUR", "ranking": [{"customer_id": ..., "name": ..., "total": "12.34"}, ...]}`.
    """
    payload = {
        "currency": currency,
        "ranking": [
            {"customer_id": customer_id, "name": name, "total": format_cents(total_cents)}
            for customer_id, name, total_cents in rows
        ],
    }
    return json.dumps(payload, ensure_ascii=False)
