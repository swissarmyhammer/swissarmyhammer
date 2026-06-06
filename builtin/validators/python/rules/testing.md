---
name: testing
description: src/ layout, mandatory test docstrings, do not mock what you do not own
severity: warn
---

# Python Testing

- **Use the `src/` layout.** Tests must run against the installed package, not the source directory. Flat layouts hide packaging bugs.
- **Test docstrings are mandatory.** Every test explains _why_ it exists, not just what it does. "Empty strings are invalid identifiers and should be rejected at the boundary, not silently produce None downstream."
- **Don't mock what you don't own.** If a test mocks `boto3.client()` or `requests.get()` directly, flag it. Mock an owned facade wrapping the dependency instead.
- **Regression tests reference bug tracker issues.** Information that doesn't fit in the test name belongs in the docstring.
