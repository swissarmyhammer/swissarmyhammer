---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffff280
title: 'Coverage: daemon.rs::file_extensions accessor'
---
## What

`swissarmyhammer-lsp/src/daemon.rs` — `file_extensions()` method has 0% coverage (FNDA:0). Simple accessor that returns extensions from a daemon spec.

## Acceptance Criteria
- [ ] `file_extensions()` has at least one test exercising it
- [ ] Test constructs a `DaemonSpec` with known extensions and asserts they are returned correctly

## Tests
- [ ] Add unit test in `swissarmyhammer-lsp/src/daemon.rs` (or `tests/`) — construct a daemon spec with known extensions, call `file_extensions()`, assert returned slice matches
- [ ] `cargo test -p swissarmyhammer-lsp file_extensions` passes

## Workflow
- Use `/tdd` — write failing test first, then verify it passes. #coverage-gap