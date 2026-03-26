---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffb180
title: 'NIT: percent_complete YAML field definition uses a fixed non-ULID id'
---
File: swissarmyhammer-kanban/builtin/fields/definitions/percent_complete.yaml line 1 — The field definition uses the id `0000000000000000000000000U`, which is not a valid ULID (not monotonic, not random). Other field definitions in the codebase use real ULIDs. Using a hand-crafted placeholder id risks collision if the ULID generation range ever includes this value and makes the intent unclear.\n\nSuggestion: generate a proper ULID for this field definition, matching the pattern used by all other builtin field YAML files.\n\nVerification step: compare the id format with the ids in adjacent builtin field definition files (e.g. position_column.yaml) and confirm they are all proper ULIDs." #review-finding