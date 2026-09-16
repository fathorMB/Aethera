"""Turn a Table into text."""

import json
from decimal import Decimal


def money(value):
    """Two decimals, rounded half-up: Decimal("2.345") -> "2.35"."""
    return str(value.quantize(Decimal("0.01"), rounding="ROUND_HALF_UP"))


def render_text(table):
    lines = []
    for g in table.groups:
        lines.append(f"{g.key:<12} {g.count:>4} {money(g.total):>12}")
    lines.append(f"{'TOTAL':<12} {sum(g.count for g in table.groups):>4} {money(table.grand_total):>12}")
    return "\n".join(lines) + "\n"


def render_json(table):
    doc = {
        "groups": [{"key": g.key, "count": g.count, "total": money(g.total)} for g in table.groups],
        "count": sum(g.count for g in table.groups),
        "grand_total": money(table.grand_total),
    }
    return json.dumps(doc) + "\n"
