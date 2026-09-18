"""The plugins shipped with this pipeline, registered under short names so
a config file can refer to them without an import path.
"""

from __future__ import annotations

from ..stages.apply_discount import apply_bulk_discount
from ..stages.filter_amount import filter_min_amount
from ..stages.filter_date_range import filter_date_range
from .registry import PluginRegistry


def register_builtin_plugins(registry: PluginRegistry) -> None:
    registry.register("filter_min_amount", filter_min_amount)
    registry.register("filter_date_range", filter_date_range)
    registry.register("apply_bulk_discount", apply_bulk_discount)
