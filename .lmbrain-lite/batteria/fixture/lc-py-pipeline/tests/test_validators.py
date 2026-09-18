import unittest

from pipeline.domain.customer import Customer
from pipeline.domain.order import Order
from pipeline.domain.product import Product
from pipeline.domain.refund import Refund
from pipeline.validators.customer_rules import validate_customer
from pipeline.validators.order_rules import validate_order
from pipeline.validators.product_rules import validate_product
from pipeline.validators.refund_rules import validate_refund
from pipeline.validators.referential import orders_with_unknown_customer, orders_with_unknown_product

CURRENCIES = {"EUR", "USD"}


class OrderRulesTest(unittest.TestCase):
    def test_valid_order_passes(self):
        order = Order("o1", "c1", "p1", 2, 500, "EUR", "2026-01-01")
        result = validate_order(order, known_currencies=CURRENCIES, max_quantity=100)
        self.assertTrue(result.valid)
        self.assertEqual(result.errors, [])

    def test_max_quantity_boundary_is_allowed(self):
        order = Order("o1", "c1", "p1", 100, 500, "EUR", "2026-01-01")
        result = validate_order(order, known_currencies=CURRENCIES, max_quantity=100)
        self.assertTrue(result.valid)

    def test_over_max_quantity_rejected(self):
        order = Order("o1", "c1", "p1", 101, 500, "EUR", "2026-01-01")
        result = validate_order(order, known_currencies=CURRENCIES, max_quantity=100)
        self.assertFalse(result.valid)

    def test_unknown_currency_rejected(self):
        order = Order("o1", "c1", "p1", 1, 500, "XYZ", "2026-01-01")
        result = validate_order(order, known_currencies=CURRENCIES, max_quantity=100)
        self.assertFalse(result.valid)

    def test_multiple_errors_all_reported(self):
        order = Order("o1", "c1", "p1", 0, -5, "XYZ", "2026-01-01")
        result = validate_order(order, known_currencies=CURRENCIES, max_quantity=100)
        self.assertEqual(len(result.errors), 3)


class CustomerRulesTest(unittest.TestCase):
    def test_valid_customer_passes(self):
        self.assertTrue(validate_customer(Customer("c1", "Ann", "US")).valid)

    def test_three_letter_country_rejected(self):
        self.assertFalse(validate_customer(Customer("c1", "Ann", "USA")).valid)

    def test_blank_name_rejected(self):
        self.assertFalse(validate_customer(Customer("c1", "  ", "US")).valid)


class ProductRulesTest(unittest.TestCase):
    def test_zero_price_allowed(self):
        self.assertTrue(validate_product(Product("p1", "Freebie", "promo", 0)).valid)

    def test_negative_price_rejected(self):
        self.assertFalse(validate_product(Product("p1", "Bad", "promo", -1)).valid)


class RefundRulesTest(unittest.TestCase):
    def test_valid_refund_passes(self):
        self.assertTrue(validate_refund(Refund("r1", "o1", 500, "damaged", "2026-01-01")).valid)

    def test_non_positive_amount_rejected(self):
        self.assertFalse(validate_refund(Refund("r1", "o1", 0, "damaged", "2026-01-01")).valid)


class ReferentialTest(unittest.TestCase):
    def test_unknown_customer_detected(self):
        orders = [Order("o1", "c1", "p1", 1, 100, "EUR", "2026-01-01"), Order("o2", "c2", "p1", 1, 100, "EUR", "2026-01-01")]
        self.assertEqual(orders_with_unknown_customer(orders, {"c1"}), ["o2"])

    def test_unknown_product_detected(self):
        orders = [Order("o1", "c1", "p1", 1, 100, "EUR", "2026-01-01")]
        self.assertEqual(orders_with_unknown_product(orders, {"p2"}), ["o1"])


if __name__ == "__main__":
    unittest.main()
