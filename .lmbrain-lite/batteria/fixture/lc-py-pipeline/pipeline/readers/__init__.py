"""Turn a source file into domain records.

Every reader in this package is a plain function `read_xxx(path) -> list[T]`;
none of them validate or normalize the records they return, that is the job
of `pipeline.validators` and `pipeline.stages`.
"""

from .csv_reader import read_orders_csv
from .jsonl_reader import read_customers_jsonl
from .product_reader import read_products_csv
from .refund_reader import read_refunds_csv

__all__ = [
    "read_orders_csv",
    "read_customers_jsonl",
    "read_products_csv",
    "read_refunds_csv",
]
