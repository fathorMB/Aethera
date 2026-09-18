"""An opt-in registry for extra stages, so a deployment can add a custom
transform (a client-specific discount, a one-off filter) without editing
`pipeline.stages` itself. Not used by `pipeline.orchestrator.run`, which
only ever calls the stages it imports directly.
"""

from .builtin import register_builtin_plugins
from .registry import PluginRegistry

__all__ = ["PluginRegistry", "register_builtin_plugins"]
