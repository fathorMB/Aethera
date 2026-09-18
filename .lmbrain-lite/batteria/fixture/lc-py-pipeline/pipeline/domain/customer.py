"""The customer record."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Customer:
    """A single customer as read from the customer feed.

    `customer_id` is the primary key used to join orders to customers in
    `pipeline.aggregators.group_by`. `country` is a two-letter ISO code; the
    normalize stage uppercases it, the feed may send it in any case.
    `segment` is a free-form label such as "retail" or "wholesale" and is
    not validated beyond being non-empty.
    """

    customer_id: str
    name: str
    country: str
    segment: str = "retail"
