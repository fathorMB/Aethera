"""Plain data records shared by every other package in `pipeline`.

None of these classes contain business logic: they only describe the shape
of a record. Rules live in `pipeline.validators`, transforms in
`pipeline.stages`.
"""

from .customer import Customer
from .order import Order
from .product import Product
from .refund import Refund

__all__ = ["Customer", "Order", "Product", "Refund"]
