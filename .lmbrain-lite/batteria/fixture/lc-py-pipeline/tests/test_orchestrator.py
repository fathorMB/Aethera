import unittest

from pipeline.config import PipelineConfig
from pipeline.domain.customer import Customer
from pipeline.domain.order import Order
from pipeline.orchestrator import run


def make_order(order_id, customer_id, quantity, unit_price_cents, currency="EUR", date="2026-01-01"):
    return Order(order_id, customer_id, "p-1", quantity, unit_price_cents, currency, date)


class OrchestratorTopCustomersTest(unittest.TestCase):
    def test_ties_break_alphabetically_by_customer_name(self):
        # Bob's order is read before Ann's, but they end up with the exact
        # same total (10.00 EUR each); the report must still list Ann
        # before Bob, because the ranking breaks ties by name, not by the
        # order the data happened to arrive in.
        customers = [
            Customer("c-bob", "Bob", "US"),
            Customer("c-ann", "Ann", "US"),
            Customer("c-top", "Zoe", "US"),
        ]
        orders = [
            make_order("o1", "c-bob", 1, 1000),
            make_order("o2", "c-ann", 1, 1000),
            make_order("o3", "c-top", 1, 5000),
        ]
        report = run(orders, customers, PipelineConfig(top_n=3))
        names_in_order = [name for _, name, _ in report.top_customers]
        self.assertEqual(names_in_order, ["Zoe", "Ann", "Bob"])

    def test_rejected_orders_are_recorded_and_excluded(self):
        customers = [Customer("c-1", "Ann", "US")]
        orders = [
            make_order("o1", "c-1", 1, 1000),
            make_order("o2", "c-1", -1, 1000),  # invalid quantity
        ]
        report = run(orders, customers, PipelineConfig(top_n=5))
        self.assertEqual(report.rejected_order_ids, ["o2"])
        self.assertEqual(report.accepted_orders, 1)


if __name__ == "__main__":
    unittest.main()
