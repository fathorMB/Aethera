"""Small generic helpers over lists and mappings, used by more than one
package (so they live here instead of next to a single caller).
"""

from __future__ import annotations

from typing import Callable, Dict, Iterable, List, TypeVar

T = TypeVar("T")
K = TypeVar("K")


def index_by(items: Iterable[T], key: Callable[[T], K]) -> Dict[K, T]:
    """`{key(item): item}`. If two items share a key, the last one wins,
    matching how a plain dict comprehension would behave.
    """
    return {key(item): item for item in items}


def chunked(items: List[T], size: int) -> List[List[T]]:
    """Split `items` into consecutive chunks of at most `size` elements.

    The last chunk may be shorter; `items` empty returns an empty list,
    never `[[]]`. Raises `ValueError` if `size` is not positive.
    """
    if size <= 0:
        raise ValueError(f"size must be positive, got {size}")
    return [items[i : i + size] for i in range(0, len(items), size)]
