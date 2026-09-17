"""Balance summaries."""

from .money import convert


def summary(accounts, currency):
    """One line per account, sorted by name, then the total converted into `currency`.

    Each account is converted on its own and the converted values are added.
    """
    lines = []
    total = 0
    for acc in sorted(accounts, key=lambda a: a.name):
        value = convert(acc.balance_cents, acc.currency, currency)
        total += value
        lines.append(f"{acc.name}: {acc.balance_cents} {acc.currency} = {value} {currency}")
    lines.append(f"TOTAL: {total} {currency}")
    return "\n".join(lines)
