"""Half-open integer intervals [start, end), as used by the booking calendar.

An interval is a tuple (start, end). start == end is an empty interval.
start > end is invalid.
"""


def validate(interval):
    start, end = interval
    if start > end:
        raise ValueError(f"invalid interval {interval!r}: start after end")
    return start, end


def merge(intervals):
    """Return the union of `intervals` as a sorted list of disjoint, non-empty intervals.

    Intervals that overlap or touch ([1, 3) and [3, 5)) are joined. Empty intervals are
    dropped. Any invalid interval raises ValueError. The input is not modified.
    """
    raise NotImplementedError


def total_length(intervals):
    """Number of integer points covered by at least one interval."""
    raise NotImplementedError


def gaps(intervals, start, end):
    """The parts of [start, end) not covered by `intervals`, sorted.

    Intervals may extend outside [start, end); only the part inside counts.
    Raises ValueError if start > end.
    """
    raise NotImplementedError
