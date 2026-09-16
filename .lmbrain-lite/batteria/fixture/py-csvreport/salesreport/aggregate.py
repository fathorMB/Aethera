"""Group rows and total them."""

from dataclasses import dataclass
from decimal import Decimal


@dataclass(frozen=True)
class Group:
    key: str
    total: Decimal
    count: int


@dataclass(frozen=True)
class Table:
    groups: list  # of Group, sorted by total descending, then key ascending
    grand_total: Decimal


def group_rows(rows, by):
    """Group by the Row attribute named `by` ("region" or "product")."""
    if by not in ("region", "product"):
        raise ValueError(f"cannot group by {by!r}")
    totals = {}
    counts = {}
    for row in rows:
        key = getattr(row, by)
        totals[key] = totals.get(key, Decimal(0)) + row.amount
        counts[key] = counts.get(key, 0) + 1
    groups = [Group(k, totals[k], counts[k]) for k in totals]
    groups.sort(key=lambda g: (-g.total, g.key))
    return Table(groups, sum((g.total for g in groups), Decimal(0)))
