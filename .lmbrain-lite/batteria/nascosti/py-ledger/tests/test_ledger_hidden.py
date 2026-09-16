import unittest

from ledger import Account, UnknownCurrency, convert, summary, transfer


class ConvertHiddenTest(unittest.TestCase):
    def test_rounding(self):
        self.assertEqual(convert(-50, "GBP", "EUR"), -59)
        self.assertEqual(convert(1, "EUR", "JPY"), 161)
        self.assertEqual(convert(150, "GBP", "EUR"), 176)  # 175.5
        self.assertEqual(convert(9_000, "EUR", "USD"), 10_000)
        self.assertEqual(convert(0, "USD", "CHF"), 0)
        self.assertIsInstance(convert(50, "GBP", "EUR"), int)

    def test_unknown_either_side(self):
        with self.assertRaises(UnknownCurrency):
            convert(1, "XYZ", "EUR")
        with self.assertRaises(ValueError):
            convert(1, "EUR", "ABC")


class TransferHiddenTest(unittest.TestCase):
    def test_gbp_to_eur(self):
        gbp = Account("g", "GBP", 50)
        eur = Account("e", "EUR", 0)
        self.assertEqual(transfer(gbp, eur, 50), 59)
        self.assertEqual((gbp.balance_cents, eur.balance_cents), (0, 59))
        self.assertEqual(eur.history, [("in", 59)])

    def test_eur_to_jpy(self):
        eur = Account("e", "EUR", 1_000)
        jpy = Account("j", "JPY", 0)
        transfer(eur, jpy, 100)
        self.assertEqual(jpy.balance_cents, 16_129)

    def test_unknown_destination_changes_nothing(self):
        eur = Account("e", "EUR", 1_000)
        odd = Account("o", "XYZ", 0)
        with self.assertRaises(UnknownCurrency):
            transfer(eur, odd, 100)
        self.assertEqual((eur.balance_cents, odd.balance_cents), (1_000, 0))
        self.assertEqual(eur.history, [])

    def test_non_positive(self):
        a, b = Account("a", "EUR", 10), Account("b", "EUR")
        with self.assertRaises(ValueError):
            transfer(a, b, 0)


class SummaryHiddenTest(unittest.TestCase):
    def test_summary_rounds_each_account(self):
        accounts = [Account("b", "GBP", 50), Account("a", "GBP", 50)]
        self.assertEqual(
            summary(accounts, "EUR"),
            "a: 50 GBP = 59 EUR\nb: 50 GBP = 59 EUR\nTOTAL: 118 EUR",
        )


if __name__ == "__main__":
    unittest.main()
