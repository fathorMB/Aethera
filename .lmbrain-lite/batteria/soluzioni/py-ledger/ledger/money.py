"""Currency conversion of integer cents."""

from decimal import ROUND_HALF_UP, Decimal

from . import rates


class UnknownCurrency(ValueError):
    """A currency code that the rate table does not know."""


def convert(cents, frm, to):
    """Convert `cents` of currency `frm` into cents of currency `to`.

    The exact result is rounded to the nearest cent, halves away from zero
    (50 GBP cents are 58.5 EUR cents, which becomes 59; -58.5 becomes -59).
    An unknown currency code raises UnknownCurrency.
    """
    for code in (frm, to):
        if code not in rates.RATES_TO_EUR:
            raise UnknownCurrency(code)
    if frm == to:
        return cents
    exact = Decimal(cents) * rates.rate(frm, to)
    return int(exact.quantize(Decimal(1), rounding=ROUND_HALF_UP))
