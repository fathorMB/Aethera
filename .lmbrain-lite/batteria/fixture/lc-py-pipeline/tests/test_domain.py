import unittest

from pipeline.domain.order import Order


class OrderTest(unittest.TestCase):
    def test_gross_cents(self):
        order = Order("o1", "c1", "p1", 3, 250, "EUR", "2026-01-01")
        self.assertEqual(order.gross_cents, 750)


if __name__ == "__main__":
    unittest.main()
