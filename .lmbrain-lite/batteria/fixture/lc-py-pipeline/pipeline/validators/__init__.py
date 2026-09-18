"""Per-record rules: a record either passes every rule whole, or is
rejected. Validators never mutate the record they check.
"""

from .customer_rules import validate_customer
from .order_rules import validate_order
from .product_rules import validate_product
from .refund_rules import validate_refund

__all__ = ["validate_customer", "validate_order", "validate_product", "validate_refund"]
