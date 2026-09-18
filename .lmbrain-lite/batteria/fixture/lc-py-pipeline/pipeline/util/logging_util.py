"""A minimal, dependency-free event log for a pipeline run.

Not Python's `logging` module: the orchestrator's tests run many small
pipelines per process and a shared, mutable `logging` configuration would
make them interfere with each other. This is just an in-memory list a
caller can inspect afterwards.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Tuple


@dataclass
class RunLog:
    """An ordered list of `(stage_name, message)` pairs collected during a
    run. Not thread-safe; each pipeline run should use its own `RunLog`.
    """

    entries: List[Tuple[str, str]] = field(default_factory=list)

    def note(self, stage_name: str, message: str) -> None:
        self.entries.append((stage_name, message))

    def for_stage(self, stage_name: str) -> List[str]:
        return [message for name, message in self.entries if name == stage_name]

    def __len__(self) -> int:
        return len(self.entries)
