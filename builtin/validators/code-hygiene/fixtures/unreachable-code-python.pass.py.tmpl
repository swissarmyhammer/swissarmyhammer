"""The passing fixture of the `unreachable-code-python` tool rule.

Every jump the failing fixture strands code behind — ``return``, ``raise``,
``continue``, and ``break`` — appears here as the last statement its branch can
run. The tool must report nothing. A tool upgrade that reports one anyway makes
the doctor mark the rule unusable.
"""


def after_return(flag: bool) -> int:
    """Return from either branch, leaving no statement behind a jump."""
    if flag:
        return 1
    return 2


def after_raise(flag: bool) -> int:
    """Raise on one branch and return on the other, both as the last step."""
    if not flag:
        raise ValueError("flag is false")
    return 1


def after_continue(items: list[int]) -> int:
    """Continue as the last statement of the branch that skips an item."""
    total = 0
    for item in items:
        if item < 0:
            continue
        total += item
    return total


def after_break(items: list[int]) -> int:
    """Break as the last statement of the branch that stops the loop."""
    total = 0
    for item in items:
        if item < 0:
            break
        total += item
    return total
