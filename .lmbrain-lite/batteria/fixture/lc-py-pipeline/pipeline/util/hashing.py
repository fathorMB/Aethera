"""A stable content hash for a batch of records, used to fingerprint a
report run (so two runs on the same input can be compared without diffing
every field).
"""

from __future__ import annotations

import hashlib
from typing import Iterable


def fingerprint(parts: Iterable[str]) -> str:
    """A short, stable hex digest of `parts`, joined with a separator byte
    that cannot appear in any part (a NUL byte), so `["ab", "c"]` and
    `["a", "bc"]` never collide.
    """
    h = hashlib.sha256()
    for part in parts:
        h.update(part.encode("utf-8"))
        h.update(b"\0")
    return h.hexdigest()[:16]
