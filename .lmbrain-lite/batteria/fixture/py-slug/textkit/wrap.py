"""Greedy word wrapping for excerpts."""


def wrap(text, width):
    """Split `text` into lines of at most `width` characters, breaking at spaces.

    A single word longer than `width` stays whole on its own line.
    """
    if width < 1:
        raise ValueError("width must be positive")
    lines = []
    current = ""
    for word in text.split():
        if not current:
            current = word
        elif len(current) + 1 + len(word) <= width:
            current += " " + word
        else:
            lines.append(current)
            current = word
    if current:
        lines.append(current)
    return lines
