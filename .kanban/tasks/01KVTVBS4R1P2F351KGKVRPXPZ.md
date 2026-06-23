---
assignees:
- claude-code
depends_on:
- 01KVTV89QPMT2Z63H2KZ8BJ3M1
- 01KVTV9QTCZ47M8TVTTY28JR6G
position_column: todo
position_ordinal: a980
project: null
title: write files — read-before-write freshness guard
---
## What
Guard `write files` against clobbering an existing file the model hasn't seen. In `crates/swissarmyhammer-tools/src/mcp/tools/files/write/mod.rs`:
- For an **existing** file, require a freshness token: a whole-file content hash from a prior `read` (the hash emitted by the read task) carried as an optional `expected_hash` param, OR session read-tracking if available on `ToolContext`.
- On divergence (token absent or not matching current on-disk content), **do not clobber** — return the current file content (hashline-tagged) so the model re-bases, rather than overwriting.
- New/nonexistent files are **unguarded** (current behavior): write freely.
- Add the `expected_hash` param to `WRITE_FILE_PARAMS` (`ParamMeta`, optional). Mirrors the hashline read-before-edit mandate and the closed-write-surface goal.
- Update `crates/swissarmyhammer-tools/src/mcp/tools/files/write/description.md` to document the freshness guard and `expected_hash`.

## Acceptance Criteria
- [ ] Writing a brand-new file requires no token and succeeds.
- [ ] Overwriting an existing file with a matching `expected_hash` succeeds.
- [ ] Overwriting an existing file with a stale/missing token returns current (tagged) content and leaves the file unchanged.
- [ ] `expected_hash` is declared in the write schema.
- [ ] `write/description.md` documents the guard + `expected_hash`.

## Tests
- [ ] Unit tests in `write/mod.rs`: new-file unguarded write; matching-hash overwrite; stale/missing-hash divergence returns current content + no clobber.
- [ ] `cargo test -p swissarmyhammer-tools write::` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.