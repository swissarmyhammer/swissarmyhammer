---
name: testing
description: src/ layout, mandatory test docstrings, do not mock what you do not own
---

# Python Testing

- **Use the `src/` layout.** Run tests against the installed package. Do not run tests against the source directory. Flat layouts hide packaging bugs.
- **Write a docstring for every test.** The docstring must explain why the test exists, not only what it does. Example: "Empty strings are invalid identifiers and should be rejected at the boundary, not silently produce None downstream."
- **Do not mock what you do not own.** Flag a test that mocks `boto3.client()` or `requests.get()` directly. Mock an owned facade that wraps the dependency instead.
- **Reference bug tracker issues in regression tests.** Put information that does not fit in the test name in the docstring.
