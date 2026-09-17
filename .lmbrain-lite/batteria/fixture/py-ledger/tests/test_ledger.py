import unittest

from ledger import Account, InsufficientFunds, UnknownCurrency, convert, summary, transfer


class ConvertTest(unittest.TestCase):
    def test_same_currency(self):
        self.assertEqual(convert(123, "EUR", "EUR"), 123)

    def test_half_cent_rounds_away_from_zero(self):
        self.assertEqual(convert(50, "GBP", "EUR"), 59)

    def test_unknown_currency(self):
        with self.assertRaises(UnknownCurrency):
            convert(100, "EUR", "XYZ")


class TransferTest(unittest.TestCase):
    def test_credit_is_in_destination_currency(self):
        eur = Account("eur", "EUR", 10_000)
        usd = Account("usd", "USD", 0)
        transfer(eur, usd, 9_000)
        self.assertEqual(eur.balance_cents, 1_000)
        self.assertEqual(usd.balance_cents, 10_000)

    def test_insufficient_funds_changes_nothing(self):
        a = Account("a", "EUR", 100)
        b = Account("b", "EUR", 0)
        with self.assertRaises(InsufficientFunds):
            transfer(a, b, 101)
        self.assertEqual((a.balance_cents, b.balance_cents), (100, 0))


class SummaryTest(unittest.TestCase):
    def test_summary(self):
        accounts = [Account("zeta", "USD", 1_000), Account("alpha", "EUR", 500)]
        self.assertEqual(
            summary(accounts, "EUR"),
            "alpha: 500 EUR = 500 EUR\nzeta: 1000 USD = 900 EUR\nTOTAL: 1400 EUR",
        )


if __name__ == "__main__":
    unittest.main()
