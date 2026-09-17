"""Load settings from defaults and environment variables.

Every setting has a default, and the default's type is the setting's type. An environment
variable named PREFIX + KEY.upper() overrides it, converted to that type:

- str: taken as is
- int: base-10 integer, surrounding whitespace allowed
- float: any float literal
- bool: "1", "true", "yes", "on" are True; "0", "false", "no", "off" are False;
  case-insensitive, surrounding whitespace allowed; anything else is an error

A value that cannot be converted raises ConfigError, and the message names the
environment variable.
"""


class ConfigError(ValueError):
    pass


def _convert(kind, raw):
    if kind is bool:
        return bool(raw)
    if kind is int:
        return int(raw)
    if kind is float:
        return float(raw)
    return raw


def load(defaults, environ, prefix="APP_"):
    """Return a new dict with every key of `defaults`, overridden from `environ`."""
    result = dict(defaults)
    for key, default in defaults.items():
        name = prefix + key.upper()
        if name not in environ:
            continue
        raw = environ[name]
        try:
            result[key] = _convert(type(default), raw)
        except ValueError as exc:
            raise ConfigError(f"{name}: cannot read {raw!r} as {type(default).__name__}") from exc
    return result
