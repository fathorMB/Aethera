"""Small helpers for the ISO-8601 date strings used everywhere in `pipeline`.

Dates are kept as plain `"YYYY-MM-DD"` strings through the whole pipeline
(see the note in `pipeline.domain.order`); this module is the only place
that ever turns one into a `datetime.date`, for the date-range filter.
"""

from __future__ import annotations

import datetime as dt


def parse_iso_date(text: str) -> dt.date:
    return dt.date.fromisoformat(text)


def in_range(date_text: str, start: str, end: str) -> bool:
    """Whether `date_text` falls within `[start, end]`, both inclusive.

    Compares the strings directly rather than parsing them: ISO-8601 dates
    of the same length sort exactly like the dates they represent, so this
    is both correct and avoids three `date.fromisoformat` calls per order.
    """
    return start <= date_text <= end
