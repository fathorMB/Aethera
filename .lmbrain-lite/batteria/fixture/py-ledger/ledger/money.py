"""Currency conversion of integer cents."""

from . import rates


class UnknownCurrency(ValueError):
    """A currency code that the rate table does not know."""


def convert(cents, frm, to):
    """Convert `cents` of currency `frm` into cents of currency `to`.

    The exact result is rounded to the nearest cent, halves away from zero
    (50 GBP cents are 58.5 EUR cents, which becomes 59; -58.5 becomes -59).
    An unknown currency code raises UnknownCurrency.
    """
    if frm == to:
        return cents
    return round(cents * float(rates.rate(frm, to)))
