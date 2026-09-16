"""Accounts and transfers between them."""

from dataclasses import dataclass, field

from .money import convert


class InsufficientFunds(Exception):
    pass


@dataclass
class Account:
    name: str
    currency: str
    balance_cents: int = 0
    history: list = field(default_factory=list)

    def deposit(self, cents):
        if cents <= 0:
            raise ValueError("deposit must be positive")
        self.balance_cents += cents
        self.history.append(("deposit", cents))


def transfer(src, dst, cents):
    """Move `cents` (in the currency of `src`) from `src` to `dst`.

    `dst` is credited with the converted amount in its own currency. Nothing changes if
    the transfer is refused: non-positive amounts raise ValueError, a balance that is too
    small raises InsufficientFunds.
    """
    if cents <= 0:
        raise ValueError("transfer must be positive")
    if src.balance_cents < cents:
        raise InsufficientFunds(src.name)
    credited = convert(cents, src.currency, dst.currency)
    src.balance_cents -= cents
    dst.balance_cents += credited
    src.history.append(("out", cents))
    dst.history.append(("in", credited))
    return credited
