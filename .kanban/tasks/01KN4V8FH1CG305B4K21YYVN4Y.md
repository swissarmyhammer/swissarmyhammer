---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9080
title: ChangeEvent lacks PartialEq, Eq, Clone for testing; ChangelogEntry lacks PartialEq
---
**swissarmyhammer-store/src/event.rs:9-15 and changelog.rs:31**\n\nPer Rust review guidelines, public types should implement all applicable standard traits.\n\n`ChangeEvent` has `Debug, Clone` but is missing `PartialEq, Eq` -- making it harder to assert on in tests.\n\n`ChangelogEntry` has `Debug, Clone, Serialize, Deserialize` but is missing `PartialEq` -- the tests never assert structural equality of entries, which weakens test coverage.\n\n`Changelog` struct has no `Debug` impl.\n\n**Severity: nit**\n\n**Suggestion:** Add `#[derive(PartialEq, Eq)]` to `ChangeEvent`, `#[derive(PartialEq)]` to `ChangelogEntry`, and `#[derive(Debug)]` to `Changelog`.\n\n**Subtasks:**\n- [ ] Add missing trait derives to ChangeEvent, ChangelogEntry, Changelog\n- [ ] Verify compilation" #review-finding