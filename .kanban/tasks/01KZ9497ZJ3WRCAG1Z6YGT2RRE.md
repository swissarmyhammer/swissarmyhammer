---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8480
title: Add project_types to ValidatorMatch with AND semantics
---
Add `project_types` as a new key on `ValidatorMatch` in `swissarmyhammer-validators/src/validators/types.rs`.

Semantics — same as the existing keys:
- The keys under `match` combine with an implicit AND. Every present key must match.
- An absent key matches everything (current ValidatorMatch behavior).
- The values inside one key combine with OR.
- So `files: ["**/*.py"]` + `project_types: [python]` = the file matches the pattern AND the workspace is a detected python project.

Work:
- Add the field to `ValidatorMatch`, serde default, so every existing manifest parses unchanged.
- Resolve detected project types from the PROJECT_TYPE_SPECS detection for the workspace under review.
- Evaluate the criterion in the one existing match code path. No second matcher.
- Update `builtin/validators/README.md` if the shipped behavior differs from the documented contract.

Acceptance:
- A match block with only `files` behaves exactly as today (regression test).
- A match block with `files` + `project_types` requires both (AND test).
- A match block with only `project_types` matches all files in a matching workspace, no files otherwise.

#tool-validators