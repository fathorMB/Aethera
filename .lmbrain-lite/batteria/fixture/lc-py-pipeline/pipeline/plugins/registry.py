"""A name -> stage-function registry.

Kept intentionally dumb (a dict with guardrails) rather than a plugin
loader that scans entry points: this pipeline runs as a single in-process
batch job, not a host for third-party packages.
"""

from __future__ import annotations

from typing import Callable, Dict, List


class PluginRegistry:
    """Register named callables and fetch them back by name.

    Registering the same name twice raises `KeyError` unless
    `replace=True` is passed, so a typo that shadows a built-in plugin is
    caught instead of silently overriding it.
    """

    def __init__(self) -> None:
        self._plugins: Dict[str, Callable] = {}

    def register(self, name: str, fn: Callable, *, replace: bool = False) -> None:
        if name in self._plugins and not replace:
            raise KeyError(f"a plugin named {name!r} is already registered")
        self._plugins[name] = fn

    def get(self, name: str) -> Callable:
        return self._plugins[name]

    def names(self) -> List[str]:
        return sorted(self._plugins)

    def __contains__(self, name: str) -> bool:
        return name in self._plugins
