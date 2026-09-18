"""Higher-level report builders: each one combines a couple of stages and
an aggregator into a single call, on top of the primitives in
`pipeline.stages` and `pipeline.aggregators`.
"""

from .category_breakdown import category_breakdown
from .region_breakdown import region_breakdown

__all__ = ["category_breakdown", "region_breakdown"]
