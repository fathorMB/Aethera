"""Application settings: typed defaults overridden by environment variables."""

from .loader import ConfigError, load

__all__ = ["ConfigError", "load"]
