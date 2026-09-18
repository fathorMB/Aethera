"""Attach a coarse region to a customer's two-letter country code.

This is used only by `pipeline.reports.region_breakdown`; the customer
ranking in `pipeline.orchestrator` does not need it. Kept as a stage of its
own so that report can reuse it without duplicating the table below.
"""

from __future__ import annotations

REGION_BY_COUNTRY = {
    "US": "Americas",
    "CA": "Americas",
    "BR": "Americas",
    "MX": "Americas",
    "GB": "Europe",
    "DE": "Europe",
    "FR": "Europe",
    "IT": "Europe",
    "ES": "Europe",
    "NL": "Europe",
    "JP": "Asia-Pacific",
    "CN": "Asia-Pacific",
    "AU": "Asia-Pacific",
    "IN": "Asia-Pacific",
}


def region_for_country(country: str) -> str:
    """The region for a two-letter country code, or `"Other"` if unknown.

    Unknown codes fall back to `"Other"` rather than raising: a region
    breakdown should still include every customer even if the country
    table has not caught up with a newly supported market.
    """
    return REGION_BY_COUNTRY.get(country.upper(), "Other")
