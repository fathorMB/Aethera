"""Exceptions raised across the pipeline package.

Every exception below carries enough context in its message to point at the
offending record without needing a debugger: the record id and, where it
makes sense, the field that was rejected.
"""

from __future__ import annotations


class PipelineError(Exception):
    """Base class for every error raised by this package."""


class ReaderError(PipelineError):
    """A source file could not be parsed into domain records."""

    def __init__(self, path: str, line_no: int, reason: str):
        self.path = path
        self.line_no = line_no
        self.reason = reason
        super().__init__(f"{path}:{line_no}: {reason}")


class ValidationError(PipelineError):
    """A single record failed one of the rules in `pipeline.validators`."""

    def __init__(self, record_id: str, field: str, reason: str):
        self.record_id = record_id
        self.field = field
        self.reason = reason
        super().__init__(f"{record_id}: {field}: {reason}")


class UnknownCurrency(PipelineError):
    """A currency code that `pipeline.util.currency` has no rate for."""

    def __init__(self, code: str):
        self.code = code
        super().__init__(f"unknown currency: {code!r}")
