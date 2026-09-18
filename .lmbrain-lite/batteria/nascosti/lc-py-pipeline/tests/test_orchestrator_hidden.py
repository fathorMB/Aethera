import unittest

from pipeline.config import PipelineConfig
from pipeline.domain.customer import Customer
from pipeline.domain.order import Order
from pipeline.orchestrator import run


def make_order(order_id, customer_id, quantity, unit_price_cents, currency="EUR", date="2026-01-01"):
    return Order(order_id, customer_id, "p-1", quantity, unit_price_cents, currency, date)


class OrchestratorHiddenTest(unittest.TestCase):
    def test_three_way_tie_reverse_insertion_order(self):
        # Orders arrive Zoe, Mia, Ann (the reverse of the expected order),
        # all with the exact same total: the ranking must still come out
        # alphabetically, not "whatever a naive reverse would produce".
        customers = [Customer("c-z", "Zoe", "US"), Customer("c-m", "Mia", "US"), Customer("c-a", "Ann", "US")]
        orders = [
            make_order("o1", "c-z", 1, 1000),
            make_order("o2", "c-m", 1, 1000),
            make_order("o3", "c-a", 1, 1000),
        ]
        report = run(orders, customers, PipelineConfig(top_n=3))
        names = [n for _, n, _ in report.top_customers]
        self.assertEqual(names, ["Ann", "Mia", "Zoe"])

    def test_top_n_cutoff_respects_tie_break(self):
        # A cutoff smaller than the number of tied entries must still keep
        # the alphabetically-first ones, not the first ones to arrive.
        customers = [Customer("c-z", "Zoe", "US"), Customer("c-m", "Mia", "US"), Customer("c-a", "Ann", "US")]
        orders = [
            make_order("o1", "c-z", 1, 1000),
            make_order("o2", "c-m", 1, 1000),
            make_order("o3", "c-a", 1, 1000),
        ]
        report = run(orders, customers, PipelineConfig(top_n=2))
        names = [n for _, n, _ in report.top_customers]
        self.assertEqual(names, ["Ann", "Mia"])

    def test_tie_between_a_ranked_and_an_unranked_customer(self):
        # A fourth customer ties with the cutoff boundary: alphabetical
        # order decides who makes the top_n, not arrival order.
        customers = [
            Customer("c-d", "Dana", "US"),
            Customer("c-c", "Cleo", "US"),
            Customer("c-hi", "High", "US"),
        ]
        orders = [
            make_order("o1", "c-d", 1, 1000),
            make_order("o2", "c-c", 1, 1000),
            make_order("o3", "c-hi", 1, 5000),
        ]
        report = run(orders, customers, PipelineConfig(top_n=2))
        names = [n for _, n, _ in report.top_customers]
        self.assertEqual(names, ["High", "Cleo"])


if __name__ == "__main__":
    unittest.main()
