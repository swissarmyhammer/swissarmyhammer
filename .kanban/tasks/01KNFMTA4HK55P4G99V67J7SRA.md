---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffd280
title: ChangelogEntry has public fields -- violates private-fields guideline
---
swissarmyhammer-store/src/changelog.rs\n\n`ChangelogEntry` is a public struct with all fields public. It is constructed by `StoreHandle` and deserialized from JSONL. Adding a field would break any code that pattern-matches or constructs it.\n\nSuggestion: Add `#[non_exhaustive]` at minimum, or make fields private with a builder/constructor. #review-finding