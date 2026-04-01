---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffe780
title: 'store.rs deserialize: body containing triple-dash on a line will corrupt frontmatter parsing'
---
swissarmyhammer-entity/src/store.rs:143

The frontmatter parser uses text.splitn(3, triple-dash) which splits on the literal substring triple-dash anywhere in the text, not just at line boundaries. A markdown body containing the string triple-dash (e.g., a horizontal rule, or a nested frontmatter block in a code fence) is safe because splitn(3,...) only splits into 3 parts. However, a file that starts with content before the first triple-dash delimiter will parse incorrectly -- parts[0] will be non-empty preamble, parts[1] will be the actual frontmatter, and parts[2] will include a leading triple-dash delimiter plus the body.

This matches io.rs behavior exactly (same splitn logic), so it is not a regression. But there is no test covering a body that contains triple-dash as a horizontal rule to verify that the round-trip is correct.

Suggestion: Add a test with body containing triple-dash (horizontal rule) to prove the splitn(3,...) approach handles it correctly. Severity: nit (matches existing behavior, test would be defensive).