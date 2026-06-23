---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvva4ed8rzjcbvwmrjp8r59g
  text: |-
    Picked up. Research done. Findings:
    - `whole_file_hash(&str)` exists in shared_utils.rs (md5 hex over full bytes); read emits `#hash:<hex>\n<body>`. Will re-derive token with it.
    - Edit tool already has a mirror pattern: structured SUCCESS results (Ambiguous/NoMatch) returned via create_success_response when it declines to write. Will mirror: missing/mismatched expected_hash on an EXISTING file returns the current content hashline-tagged (via swissarmyhammer_hashline::tag) as a SUCCESS, no clobber.
    - rg execute_write callers: ONLY the dispatcher in files/mod.rs (op "write file" + content-inference branch) and in-module tests. No internal non-model programmatic caller of execute_write exists — other tools write via SecureFileAccess::write / std::fs::write directly, which bypass the guard. So no internal caller is blocked by the guard. Will confirm against integration tests too.
    - Regression: in-module overwrite tests (test_write_overwrite_existing_file) + integration tests that overwrite existing files w/o expected_hash will now no-clobber. Will update them deliberately.
  timestamp: 2026-06-23T22:38:07.272010+00:00
- actor: claude-code
  id: 01kvvafnfs38s7kwh6qx42avxe
  text: |-
    Implementation landed (TDD: wrote 4 failing tests, watched RED clobber the file, implemented guard, GREEN).
    Changes:
    - write/mod.rs: added `expected_hash` (optional String) to WRITE_FILE_PARAMS + WriteRequest; added module-level `freshness_rebase(path, expected_hash) -> Result<Option<String>>` helper + HASH_LINE_PREFIX const; guard runs in execute_write only when validated_path.exists(): matching hash -> proceed; absent/stale -> return current content as SUCCESS (`#hash:<token>\n` + swissarmyhammer_hashline::tag, mirroring read files), no clobber. New file -> unguarded.
    - write/description.md: documented the guard + expected_hash + re-base return.
    - Tests: 4 new unit tests (new-file unguarded, matching-hash overwrite, missing-token no-clobber, stale-token no-clobber). Updated existing unit test_write_overwrite_existing_file to present the matching token. Added integration test_write_tool_existing_file_without_token_does_not_clobber (production dispatcher path) + updated integration test_write_tool_overwrite_existing_file to pass expected_hash.

    Behavior matrix:
    - new/nonexistent: writes freely regardless of expected_hash.
    - existing + matching expected_hash: overwrites.
    - existing + missing expected_hash: NO clobber, returns current content tagged.
    - existing + stale expected_hash: NO clobber, returns current content tagged with the CURRENT token.
    - existing read-only file: still errors at check_file_permissions (guard runs after, unchanged).
    - existing binary/non-UTF-8 file: freshness_rebase read_to_string errors (consistent with read path) -> error, not silent clobber.

    Internal-caller finding: rg execute_write across workspace -> only files/mod.rs dispatcher + tests. No internal non-model programmatic caller of execute_write exists. Other tools that write programmatically use SecureFileAccess::write / std::fs::write directly, which do NOT route through execute_write and are NOT affected by the guard. So no internal caller is blocked.

    Verification (all GREEN):
    - cargo nextest run -p swissarmyhammer-tools write:: files:: -> 237 passed
    - cargo nextest run -p swissarmyhammer-tools (full lib+integration) -> 1371 passed, 0 failed
    - cargo test -p swissarmyhammer-tools --doc -> 8 passed
    - cargo clippy -p swissarmyhammer-tools -- -D warnings -> clean
    - cargo fmt -p swissarmyhammer-tools applied
    Prior test changed: test_write_overwrite_existing_file (unit) + test_write_tool_overwrite_existing_file (integration) now pass expected_hash, because an existing-file overwrite with no token now (correctly) no-clobbers per the task.
  timestamp: 2026-06-23T22:44:14.969203+00:00
- actor: claude-code
  id: 01kvvak9pey64q01jnem6696dd
  text: Adversarial double-check returned PASS (verified guard ordering, TOCTOU-as-designed, new-file unguarded, payload byte-identical to read-files output, binary/non-UTF-8 errors propagate via ? rather than clobbering, no internal caller blocked). Moved to review. Not committed.
  timestamp: 2026-06-23T22:46:13.966736+00:00
depends_on:
- 01KVTV89QPMT2Z63H2KZ8BJ3M1
- 01KVTV9QTCZ47M8TVTTY28JR6G
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffda80
project: file-edit-tools
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