---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffe80
title: PerspectiveContext dual-path (StoreHandle vs direct I/O) adds maintenance burden
---
swissarmyhammer-perspectives/src/context.rs -- write() and delete()\n\nBoth `write()` and `delete()` have two code paths: one that delegates to a `StoreHandle` (when wired in) and a fallback that does direct file I/O. This dual-path design means every future change to write/delete behavior must be implemented twice and kept in sync.\n\nThe fallback path does not produce change events, does not record changelog entries, and does not support undo/redo. Tests that use the fallback path are not testing the production path.\n\nSuggestion: Consider always requiring a `StoreHandle` (set during construction or `open()`). If a lightweight path is needed for tests, provide a mock or in-memory `TrackedStore` implementation instead of duplicating the I/O logic. #review-finding