import unittest
from pathlib import Path

from pipeline.errors import ReaderError
from pipeline.readers.csv_reader import read_orders_csv
from pipeline.readers.jsonl_reader import read_customers_jsonl
from pipeline.readers.product_reader import read_products_csv
from pipeline.readers.refund_reader import read_refunds_csv

FIXTURES = Path(__file__).parent / "fixtures"


class CsvReaderTest(unittest.TestCase):
    def test_reads_every_row(self):
        orders = read_orders_csv(FIXTURES / "orders_sample.csv")
        self.assertEqual(len(orders), 4)
        self.assertEqual(orders[0].order_id, "o-1")
        self.assertEqual(orders[0].quantity, 2)
        self.assertEqual(orders[0].unit_price_cents, 1500)

    def test_missing_field_raises(self):
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "bad.csv"
            path.write_text("order_id,customer_id\no1,c1\n", encoding="utf-8")
            with self.assertRaises(ReaderError):
                read_orders_csv(path)


class JsonlReaderTest(unittest.TestCase):
    def test_reads_every_line(self):
        customers = read_customers_jsonl(FIXTURES / "customers_sample.jsonl")
        self.assertEqual(len(customers), 3)
        self.assertEqual(customers[1].customer_id, "c-bob")
        self.assertEqual(customers[1].segment, "retail")  # default applied


class ProductReaderTest(unittest.TestCase):
    def test_reads_every_row(self):
        products = read_products_csv(FIXTURES / "products_sample.csv")
        self.assertEqual(len(products), 3)
        self.assertEqual(products[0].category, "hardware")


class RefundReaderTest(unittest.TestCase):
    def test_reads_every_row_blank_reason_allowed(self):
        refunds = read_refunds_csv(FIXTURES / "refunds_sample.csv")
        self.assertEqual(len(refunds), 2)
        self.assertEqual(refunds[1].reason, "")


if __name__ == "__main__":
    unittest.main()
