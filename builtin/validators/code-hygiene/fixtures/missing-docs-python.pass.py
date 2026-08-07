"""The passing fixture of the `missing-docs-python` tool rule.

Every public item here has documentation. The tool must report nothing. A
tool upgrade that reports one anyway makes the doctor mark the rule unusable.
"""


class DocumentedClass:
    """A documented public class."""

    def documented_method(self) -> None:
        """A documented public method."""


def documented_function() -> None:
    """A documented public function."""
