"""A tiny helper for chaining stages, used by callers that want to splice
in optional stages (see `pipeline.plugins`) without editing the
orchestrator.
"""

from __future__ import annotations

from typing import Callable, List, TypeVar

T = TypeVar("T")
Stage = Callable[[List[T]], List[T]]


def apply_stages(records: List[T], stages: List[Stage]) -> List[T]:
    """Run `records` through `stages` in order, feeding each stage's output
    to the next.
    """
    for stage in stages:
        records = stage(records)
    return records
