---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffac80
title: 'NIT: virtual_tags.yaml and filter_tags.yaml use hardcoded ULID-like sentinel IDs'
---
**File:** swissarmyhammer-kanban/builtin/fields/definitions/virtual_tags.yaml, filter_tags.yaml\n\n**What:** The field definition files use IDs `0000000000000000000000000W` and `0000000000000000000000000X`. These are not valid ULIDs (ULIDs encode timestamp + randomness, never all-zeros). They appear to be hand-crafted sentinel values to ensure they sort before real ULIDs.\n\n**Why this matters:** If any code validates that field definition IDs are proper ULIDs, these will fail. Also, the pattern of incrementing the last character (W, X) is fragile -- it is not documented what range is reserved for builtins vs user-defined fields.\n\n**Suggestion:** Add a comment in the YAML files (or in defaults.rs) explaining the sentinel ID convention and the reserved range. Consider using a documented prefix pattern like `00000000000000000000000` + two-char builtin code.\n\n**Verification:** No test needed -- documentation concern." #review-finding