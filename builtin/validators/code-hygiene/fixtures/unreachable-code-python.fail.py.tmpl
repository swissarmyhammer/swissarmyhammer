"""The failing fixture of the `unreachable-code-python` tool rule.

This file holds one unreachable statement after every jump the passing fixture
uses — ``return``, ``raise``, ``continue``, and ``break``. The tool must report
each one. A tool upgrade that stops reporting a jump makes the doctor mark the
rule unusable.
"""


def after_return() -> int:
    """Return a value, then hold a statement the function can never run."""
    return 1
    print("after return")


def after_raise() -> int:
    """Raise an error, then hold a statement the function can never run."""
    raise ValueError("always")
    print("after raise")


def after_continue(items: list[int]) -> int:
    """Continue the loop, then hold a statement the loop can never run."""
    total = 0
    for item in items:
        continue
        total += item
    return total


def after_break(items: list[int]) -> int:
    """Break the loop, then hold a statement the loop can never run."""
    total = 0
    for item in items:
        break
        total += item
    return total
