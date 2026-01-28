---
name: code-duplication
description: Detect duplicate code blocks and similar logic patterns
severity: error
trigger: Stop
tags:
  - code-quality
  - maintainability
  - refactoring
timeout: 30
---

Check code for duplicated code blocks and similar logic patterns.
Only look at this code, there is no need to load additional files.

Look for:
- Identical or near-identical code blocks (>5 lines)
- Similar algorithms or business logic that could be abstracted
- Repeated constant values or configuration
- Duplicate test setup or assertion patterns

Suggest refactoring through:
- Extracting shared functions or methods
- Creating utility modules or helpers
- Defining shared constants or configuration
- Using parametric patterns or generics

Do not flag:
- Boilerplate required by the language or framework
- Code that is similar but serves different domains
- Small snippets (<5 lines) that are common patterns

## Response Format

Return JSON in this exact format:

```json
{
  "status": "passed",
  "message": "No significant code duplication detected"
}
```

Or if duplications are found:

```json
{
  "status": "failed",
  "message": "Found code duplication - Lines 42-58 and 120-136 contain nearly identical validation logic; suggest extracting to `validate_input()` function"
}
```

Report duplications with:
- Location of duplicate blocks (file and line numbers)
- Similarity description
- Specific refactoring suggestion
