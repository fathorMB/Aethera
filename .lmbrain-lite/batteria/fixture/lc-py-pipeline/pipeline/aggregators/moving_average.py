"""A simple moving average over a series of order totals, in the order
they are given (the caller is expected to have sorted them, typically by
date with `pipeline.stages.sort_stage.sort_orders_by_date`).
"""

from __future__ import annotations

from typing import List


def moving_average(values: List[int], window: int) -> List[float]:
    """The moving average of `values` with the given `window` size.

    Returns one value per input value: for the first `window - 1`
    positions, the average is taken over however many values are
    available so far (a "growing" window), not padded with zeros. Raises
    `ValueError` if `window` is not positive.
    """
    if window <= 0:
        raise ValueError(f"window must be positive, got {window}")
    result: List[float] = []
    running_sum = 0
    for i, value in enumerate(values):
        running_sum += value
        start = max(0, i - window + 1)
        if i >= window:
            running_sum -= values[i - window]
        count = i - start + 1
        result.append(running_sum / count)
    return result
