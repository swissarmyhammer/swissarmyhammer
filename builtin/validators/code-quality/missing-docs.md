---
name: missing-docs
description: Check that public functions and types have documentation comments
severity: error
trigger: Stop
tags:
  - code-quality
  - documentation
timeout: 30
---

Check code for public functions, methods, structs, and types that lack documentation comments.

Look for:
- Public functions without doc comments
- Public structs/classes without doc comments
- Public enums without doc comments
- Complex public APIs without usage examples

Do not flag:
- Private/internal items
- Test functions
- Obvious implementations (e.g., Display, Debug derives)
- Generated code
