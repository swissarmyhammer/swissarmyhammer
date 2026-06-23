---
assignees:
- claude-code
depends_on:
- 01KVTVACR1W8HFKFR8DTAJPMP0
position_column: todo
position_ordinal: ab80
project: file-edit-tools
title: edit files — idempotency, no-op rejection, and consumed-target detection
---
## What
Follow-on to the cascade core. Add the safety/idempotency semantics to `edit files` in `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs`, layered on the resolved-pair apply loop.

- **No-op rejection**: reject a pair where `find == replace`.
- **Already-applied detection**: if a pair's `replace` text is already present in the file and `find` is absent, report "likely already applied" (informational, not a hard error) rather than failing with "not found".
- **Consumed-target detection**: in a multi-pair batch, detect when a later pair's resolved target span was consumed/overwritten by an earlier pair in the same batch, and report that specific case (per-edit) instead of a generic miss. (Doc line 211.)

These operate on the atomic batch from the cascade-core task, so they must run during the resolve-all phase before commit; no double-apply.

## Acceptance Criteria
- [ ] No-op (`find == replace`) is rejected with a clear message.
- [ ] `replace` present + `find` absent reports "already applied" (not a hard "not found" error).
- [ ] A later pair whose target was consumed by an earlier pair in the same batch is detected and reported per-edit; the batch remains atomic (file byte-identical on failure).

## Tests
- [ ] Unit tests: no-op rejection; already-applied path; consumed-target detection with byte-identical file on the failing batch.
- [ ] `cargo test -p swissarmyhammer-tools edit::` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.