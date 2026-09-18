"""Currency conversion for integer-cents amounts.

The rate table is illustrative, fixed at build time (a real deployment
would refresh it from a feed), and expressed as "units of `to` per unit of
`frm`" for every ordered pair the pipeline supports.
"""

from __future__ import annotations

from ..errors import UnknownCurrency

_RATES: dict[tuple[str, str], float] = {
    ("EUR", "USD"): 1.08,
    ("USD", "EUR"): 0.93,
    ("EUR", "GBP"): 0.86,
    ("GBP", "EUR"): 1.16,
    ("USD", "GBP"): 0.80,
    ("GBP", "USD"): 1.25,
    ("EUR", "JPY"): 161.29,
    ("JPY", "EUR"): 0.0062,
    ("USD", "JPY"): 149.20,
    ("JPY", "USD"): 0.0067,
}

KNOWN_CURRENCIES = {"EUR", "USD", "GBP", "JPY"}


def known_currencies() -> set[str]:
    return set(KNOWN_CURRENCIES)


def convert(cents: int, frm: str, to: str) -> int:
    """Convert `cents` of currency `frm` into cents of currency `to`.

    The exact result is rounded to the nearest cent, halves away from
    zero, matching how a bank statement rounds a converted amount. Raises
    `UnknownCurrency` if either code is not in `KNOWN_CURRENCIES`.
    """
    if frm not in KNOWN_CURRENCIES:
        raise UnknownCurrency(frm)
    if to not in KNOWN_CURRENCIES:
        raise UnknownCurrency(to)
    if frm == to:
        return cents
    rate = _RATES[(frm, to)]
    exact = cents * rate
    if exact >= 0:
        return int(exact + 0.5)
    return -int(-exact + 0.5)
