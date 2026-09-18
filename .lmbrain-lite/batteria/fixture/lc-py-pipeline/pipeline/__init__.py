"""pipeline — a small batch pipeline for turning order feeds into a sales report.

See README.md for the shape of the package and the order the stages run in.
"""

from .orchestrator import Report, run

__all__ = ["Report", "run"]
