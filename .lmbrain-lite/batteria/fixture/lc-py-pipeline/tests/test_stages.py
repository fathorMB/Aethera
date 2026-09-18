import unittest

from pipeline.domain.customer import Customer
from pipeline.domain.order import Order
from pipeline.stages.apply_discount import apply_bulk_discount
from pipeline.stages.dedupe import dedupe_orders
from pipeline.stages.enrich_currency import enrich_orders
from pipeline.stages.enrich_geo import region_for_country
from pipeline.stages.filter_amount import filter_min_amount
from pipeline.stages.filter_date_range import filter_date_range
from pipeline.stages.normalize import normalize_customers, normalize_orders
from pipeline.stages.sort_stage import sort_orders_by_date


class NormalizeTest(unittest.TestCase):
    def test_order_ids_trimmed_currency_uppercased(self):
        order = Order(" o1 ", " c1 ", " p1 ", 1, 100, "eur", "2026-01-01")
        [normalized] = normalize_orders([order])
        self.assertEqual(normalized.order_id, "o1")
        self.assertEqual(normalized.currency, "EUR")

    def test_customer_country_uppercased(self):
        [normalized] = normalize_customers([Customer("c1", " Ann ", "us")])
        self.assertEqual(normalized.name, "Ann")
        self.assertEqual(normalized.country, "US")


class DedupeTest(unittest.TestCase):
    def test_keeps_first_occurrence(self):
        first = Order("o1", "c1", "p1", 1, 100, "EUR", "2026-01-01")
        dup = Order("o1", "c2", "p2", 9, 900, "USD", "2026-02-02")
        second = Order("o2", "c1", "p1", 1, 100, "EUR", "2026-01-02")
        result = dedupe_orders([first, dup, second])
        self.assertEqual([o.order_id for o in result], ["o1", "o2"])
        self.assertEqual(result[0].customer_id, "c1")  # the first o1, not the duplicate


class SortStageTest(unittest.TestCase):
    def test_sorts_ascending_by_date(self):
        orders = [
            Order("o3", "c1", "p1", 1, 100, "EUR", "2026-03-01"),
            Order("o1", "c1", "p1", 1, 100, "EUR", "2026-01-01"),
            Order("o2", "c1", "p1", 1, 100, "EUR", "2026-02-01"),
        ]
        result = sort_orders_by_date(orders)
        self.assertEqual([o.order_id for o in result], ["o1", "o2", "o3"])


class FilterDateRangeTest(unittest.TestCase):
    def test_none_keeps_everything(self):
        orders = [Order("o1", "c1", "p1", 1, 100, "EUR", "2026-01-01")]
        self.assertEqual(filter_date_range(orders, None), orders)

    def test_inclusive_bounds(self):
        orders = [
            Order("o1", "c1", "p1", 1, 100, "EUR", "2026-01-01"),
            Order("o2", "c1", "p1", 1, 100, "EUR", "2026-01-05"),
            Order("o3", "c1", "p1", 1, 100, "EUR", "2026-01-10"),
        ]
        result = filter_date_range(orders, ("2026-01-01", "2026-01-05"))
        self.assertEqual([o.order_id for o in result], ["o1", "o2"])


class FilterAmountTest(unittest.TestCase):
    def test_keeps_at_or_above_floor(self):
        orders = [
            Order("o1", "c1", "p1", 1, 100, "EUR", "2026-01-01"),
            Order("o2", "c1", "p1", 1, 500, "EUR", "2026-01-01"),
        ]
        result = filter_min_amount(orders, 500)
        self.assertEqual([o.order_id for o in result], ["o2"])


class ApplyDiscountTest(unittest.TestCase):
    def test_applies_best_tier(self):
        order = Order("o1", "c1", "p1", 20, 1000, "EUR", "2026-01-01")
        [discounted] = apply_bulk_discount([order])
        self.assertEqual(discounted.unit_price_cents, 900)  # 10% off

    def test_below_smallest_tier_unchanged(self):
        order = Order("o1", "c1", "p1", 3, 1000, "EUR", "2026-01-01")
        [discounted] = apply_bulk_discount([order])
        self.assertEqual(discounted.unit_price_cents, 1000)


class EnrichCurrencyTest(unittest.TestCase):
    def test_same_currency_total_unchanged(self):
        order = Order("o1", "c1", "p1", 2, 1000, "EUR", "2026-01-01")
        [enriched] = enrich_orders([order], "EUR")
        self.assertEqual(enriched.total_target_cents, 2000)


class EnrichGeoTest(unittest.TestCase):
    def test_known_country(self):
        self.assertEqual(region_for_country("DE"), "Europe")

    def test_unknown_country_falls_back(self):
        self.assertEqual(region_for_country("ZZ"), "Other")


if __name__ == "__main__":
    unittest.main()
