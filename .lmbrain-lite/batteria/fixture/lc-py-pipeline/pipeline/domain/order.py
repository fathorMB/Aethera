"""The order record: the main unit of work for this pipeline."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass
class Order:
    """One line item sold to a customer.

    `unit_price_cents` and `currency` are as they were at the time of sale;
    they are not looked up from the product catalog. `order_date` is an
    ISO-8601 date string (`"YYYY-MM-DD"`), never a `datetime`: the pipeline
    only ever compares and sorts dates as strings, which works because
    ISO-8601 dates sort lexicographically in calendar order.
    """

    order_id: str
    customer_id: str
    product_id: str
    quantity: int
    unit_price_cents: int
    currency: str
    order_date: str

    @property
    def gross_cents(self) -> int:
        """Quantity times unit price, in the order's own currency."""
        return self.quantity * self.unit_price_cents
