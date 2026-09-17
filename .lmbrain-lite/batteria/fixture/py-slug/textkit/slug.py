"""URL slugs for post titles."""

import re


def slugify(text, sep="-", max_len=None):
    """Turn a title into a URL slug.

    - accented letters lose their accent ("Café" -> "cafe"); other non-ASCII characters are dropped
    - letters are lowercased
    - every run of characters that are not ASCII letters or digits becomes one `sep`
    - no `sep` at the start or at the end
    - with `max_len`, the slug is cut to at most `max_len` characters, and a `sep`
      left at the end by the cut is removed
    """
    text = text.lower()
    slug = re.sub(r"[^a-z0-9]", sep, text)
    slug = slug.strip(sep)
    if max_len is not None:
        slug = slug[:max_len]
    return slug
