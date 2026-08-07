"""The failing fixture of the `missing-docs-python` tool rule.

It holds one undocumented public item. The tool must report it. A tool
upgrade that stops reporting it makes the doctor mark the rule unusable.
"""


def documented_neighbor() -> None:
    """A documented public function, so only the item below is reported."""


def undocumented_function() -> None:
    return None
