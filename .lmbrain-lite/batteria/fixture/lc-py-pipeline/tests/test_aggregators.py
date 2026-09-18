import unittest

from pipeline.aggregators.category_totals import totals_by_category
from pipeline.aggregators.group_by import totals_by_customer
from pipeline.aggregators.histogram import histogram
from pipeline.aggregators.moving_average import moving_average
from pipeline.aggregators.top_n import top_n_by_total
from pipeline.aggregators.totals import grand_total
from pipeline.domain.order import Order
from pipeline.domain.product import Product
from pipeline.stages.enrich_currency import EnrichedOrder


def eo(order_id, customer_id, total_cents, product_id="p1"):
    return EnrichedOrder(Order(order_id, customer_id, product_id, 1, total_cents, "EUR", "2026-01-01"), total_cents)


class GroupByTest(unittest.TestCase):
    def test_sums_per_customer(self):
        totals = totals_by_customer([eo("o1", "c1", 100), eo("o2", "c1", 50), eo("o3", "c2", 30)])
        self.assertEqual(totals, {"c1": 150, "c2": 30})


class TopNTest(unittest.TestCase):
    def test_truncates_to_n(self):
        ranked = top_n_by_total({"c1": 300, "c2": 100, "c3": 200}, 2)
        self.assertEqual(ranked, [("c1", 300), ("c3", 200)])

    def test_n_larger_than_entries_returns_all(self):
        ranked = top_n_by_total({"c1": 10}, 5)
        self.assertEqual(ranked, [("c1", 10)])

    def test_n_zero_returns_empty(self):
        self.assertEqual(top_n_by_total({"c1": 10}, 0), [])


class GrandTotalTest(unittest.TestCase):
    def test_sums_every_order(self):
        self.assertEqual(grand_total([eo("o1", "c1", 100), eo("o2", "c2", 50)]), 150)


class CategoryTotalsTest(unittest.TestCase):
    def test_groups_by_product_category(self):
        products = [Product("p1", "Widget", "hardware", 1000), Product("p2", "Plan", "services", 500)]
        totals = totals_by_category([eo("o1", "c1", 100, "p1"), eo("o2", "c1", 50, "p2")], products)
        self.assertEqual(totals, {"hardware": 100, "services": 50})

    def test_unknown_product_falls_back(self):
        totals = totals_by_category([eo("o1", "c1", 100, "p-missing")], [])
        self.assertEqual(totals, {"uncategorized": 100})


class HistogramTest(unittest.TestCase):
    def test_buckets_by_width(self):
        orders = [eo("o1", "c1", 5), eo("o2", "c1", 15), eo("o3", "c1", 25)]
        result = histogram(orders, 10)
        self.assertEqual(result, {(0, 10): 1, (10, 20): 1, (20, 30): 1})

    def test_rejects_non_positive_width(self):
        with self.assertRaises(ValueError):
            histogram([], 0)


class MovingAverageTest(unittest.TestCase):
    def test_growing_window_at_the_start(self):
        self.assertEqual(moving_average([10, 20, 30], 2), [10.0, 15.0, 25.0])

    def test_rejects_non_positive_window(self):
        with self.assertRaises(ValueError):
            moving_average([1, 2], 0)


if __name__ == "__main__":
    unittest.main()
