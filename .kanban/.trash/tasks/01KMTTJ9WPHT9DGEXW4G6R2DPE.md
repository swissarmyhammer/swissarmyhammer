---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
title: '[WARNING] MergeConflict type conflates parse errors with actual merge conflicts'
---
`swissarmyhammer-merge/src/yaml.rs:160-168`, `swissarmyhammer-merge/src/md.rs:83-85`\n\n`MergeConflict::conflicting_ids` is reused to carry two semantically different payloads: (1) parse error messages such as \"YAML parse error: ...\" and (2) conflict marker text for actual merge conflicts. This conflation makes it impossible for CLI driver callers to distinguish a parse failure from a genuine three-way conflict, which matters for exit code selection (exit 1 for errors, exit 2 for unresolvable conflicts is the standard git merge driver contract). A caller that receives `MergeConflict` today cannot know whether to report a bug or leave conflict markers in the file for the user to resolve.\n\nIntroduce a proper error enum (e.g., `MergeError::ParseFailure(String)` vs `MergeError::Conflict(MergeConflict)`) so the CLI drivers can branch on the variant and emit the correct exit code." #review-finding