import unittest

from pipeline.domain.customer import Customer
from pipeline.domain.product import Product
from pipeline.plugins.builtin import register_builtin_plugins
from pipeline.plugins.registry import PluginRegistry
from pipeline.reports.category_breakdown import category_breakdown
from pipeline.reports.region_breakdown import region_breakdown
from pipeline.domain.order import Order
from pipeline.stages.enrich_currency import EnrichedOrder


def eo(order_id, customer_id, total_cents, product_id="p1"):
    return EnrichedOrder(Order(order_id, customer_id, product_id, 1, total_cents, "EUR", "2026-01-01"), total_cents)


class PluginRegistryTest(unittest.TestCase):
    def test_register_and_get(self):
        registry = PluginRegistry()
        registry.register("double", lambda x: x * 2)
        self.assertEqual(registry.get("double")(21), 42)

    def test_duplicate_name_raises(self):
        registry = PluginRegistry()
        registry.register("a", lambda: None)
        with self.assertRaises(KeyError):
            registry.register("a", lambda: None)

    def test_builtin_plugins_registered(self):
        registry = PluginRegistry()
        register_builtin_plugins(registry)
        self.assertIn("filter_min_amount", registry.names())
        self.assertIn("apply_bulk_discount", registry.names())


class RegionBreakdownTest(unittest.TestCase):
    def test_groups_by_region(self):
        customers = [Customer("c1", "Ann", "US"), Customer("c2", "Bob", "DE")]
        totals = region_breakdown([eo("o1", "c1", 100), eo("o2", "c2", 50)], customers)
        self.assertEqual(totals, {"Americas": 100, "Europe": 50})

    def test_unknown_customer_falls_back_to_other(self):
        totals = region_breakdown([eo("o1", "c-missing", 100)], [])
        self.assertEqual(totals, {"Other": 100})


class CategoryBreakdownTest(unittest.TestCase):
    def test_ranked_highest_first(self):
        products = [Product("p1", "Widget", "hardware", 1000), Product("p2", "Plan", "services", 500)]
        ranked = category_breakdown([eo("o1", "c1", 50, "p2"), eo("o2", "c1", 200, "p1")], products)
        self.assertEqual(ranked, [("hardware", 200), ("services", 50)])


if __name__ == "__main__":
    unittest.main()
