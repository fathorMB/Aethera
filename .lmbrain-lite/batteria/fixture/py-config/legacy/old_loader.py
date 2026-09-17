# FROZEN: compatibility shim for the 1.x plugins.
#
# This file must stay byte-identical to the one shipped in 1.x: the plugin loader checks
# its SHA-256 before importing it, and the 1.x plugins rely on its exact behaviour,
# including the way it reads booleans. Do not modify it, even to fix bugs.

import os


def get_setting(key, default):
    raw = os.environ.get("APP_" + key.upper())
    if raw is None:
        return default
    if isinstance(default, bool):
        return bool(raw)
    if isinstance(default, int):
        return int(raw)
    return raw
