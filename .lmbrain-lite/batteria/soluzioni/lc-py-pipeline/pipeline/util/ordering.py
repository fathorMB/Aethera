"""Ranking helper shared by `pipeline.aggregators.top_n`.

`rank_totals` turns a `{label: total}` mapping into a ranked list, highest
total first. Ties (two labels with the exact same total) must break in
ascending alphabetical order of the label, so that the same input always
produces the same report no matter what order the records that built
`totals` happened to arrive in — reports must be reproducible, not just
"correct on average".
"""

from __future__ import annotations

from typing import Dict, List, Tuple


def rank_totals(totals: Dict[str, int]) -> List[Tuple[str, int]]:
    """Rank `totals` from the highest value to the lowest.

    See the module docstring for the tie-breaking rule.
    """
    return sorted(totals.items(), key=lambda item: (-item[1], item[0]))
