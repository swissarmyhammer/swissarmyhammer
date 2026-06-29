---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw9qcg6w0s3nb9sy592dey5c
  text: |-
    Implemented. Removed the read-before-write freshness guard from the write op:
    - write/mod.rs: deleted the `if validated_path.exists()` guard block, `freshness_rebase` fn, `HASH_LINE_PREFIX` const, the `expected_hash` field on WriteRequest, and the `expected_hash` ParamMeta entry. Full-file write now always clobbers via the same unguarded path; successful overwrite still carries the mutation envelope.
    - Grep for write-op `expected_hash` senders: the ONLY callers were two tests in tests/integration/file_tools_integrations.rs (test_write_tool_overwrite_existing_file + test_write_tool_existing_file_without_token_does_not_clobber). No grammar/runtime caller sends it. Other `expected_hash`/HASH_LINE_PREFIX matches are the read op `#hash:` token and the edit op hashline anchor — both unrelated, left intact. Field removed entirely (serde ignores any stray key, so off-grammar `expected_hash` is silently dropped, never influences behavior).
    - Tests: updated test_write_overwrite_existing_file (no hash, asserts envelope); deleted the 3 guard tests + guard_divergence test + create_test_arguments_with_hash + result_text helper. Updated integration test to test_write_tool_existing_file_clobbers_without_token.
    - Docs: rewrote write/description.md, ideas/file-edit-tools.md write note, and the read/description.md reference.

    Verification: `cargo nextest run -p swissarmyhammer-tools` → 1409 passed, 0 failed.
  timestamp: 2026-06-29T12:59:04.796945+00:00
- actor: claude-code
  id: 01kw9qg6cgmmnzj1nt32km4gsb
  text: 'Double-check found write/description.md was NOT actually updated despite my earlier `files write` op reporting success. Root cause (and a live demonstration of the very bug this task fixes): the running sah MCP server still has the OLD guarded build of the `write` op loaded. My `write file` calls to that exact description.md were silently swallowed by the stale read-before-write guard — it returned the file''s current (old) content as a SUCCESS without writing, which I misread as confirmation. Rewrote the file via the shell `cat >` heredoc to bypass the MCP write op. Verified on disk: no freshness-guard section, no expected_hash, no divergence-return note. git status now shows all 5 expected files modified. (Other files edited fine because `edit file` uses a different op path.)'
  timestamp: 2026-06-29T13:01:05.808816+00:00
- actor: claude-code
  id: 01kw9qgvfkvfpv2zsrhpbqk273
  text: 'Orchestrator: implement landed green in doing — `nextest -p swissarmyhammer-tools` 1409 passed / 0 failed; all 5 expected files modified (write/mod.rs, write/description.md, read/description.md, file_tools_integrations.rs, ideas/file-edit-tools.md). Proceeding to checkpoint commit, then review.'
  timestamp: 2026-06-29T13:01:27.411910+00:00
position_column: doing
position_ordinal: '8280'
project: file-edit-tools
title: write files should clobber existing files — drop (or escape-hatch) the read-before-write freshness guard
---
## Problem

`write files` (the `write` op on the `files` MCP tool) refuses to overwrite an existing file unless the caller passes a matching `expected_hash` freshness token. When the token is **absent or stale**, the tool does NOT write — instead it returns the file's *current* content (hashline-tagged, led by a `#hash:` line) as a **SUCCESS** (`is_error: false`, no `bytes_written`).

To a calling agent this is indistinguishable from a successful write at a glance and reads like the tool is broken. A real agent report:

> "The write file op is returning the file unchanged (read-style output, no mutation/bytes_written) — it looks like it's refusing to clobber the existing file. Let me try forcing overwrite."

The agent had no `force` option and was left confused. Agents that call `write files` on an existing path overwhelmingly *intend* to clobber (regenerating a whole file). We have source control as the safety net for lost work, so the lost-update protection the guard buys is not worth the friction + confusion it creates.

## Decision

**A full-file write must NOT hash-check at all.** `write files` always replaces the target — new or existing, token or no token — with the same unguarded code path. There is no freshness check, no `expected_hash`, no re-base return, no `force` flag (none is needed because the default IS the clobber). The whole point of `write` is whole-file replacement; source control is the recovery path. (Lost-update protection still exists where it belongs: line-anchored `edit files`, via hashline.)

## Current design to REMOVE (this is deliberate, documented behavior — tear it out for the write op)

- Impl: `crates/swissarmyhammer-tools/src/mcp/tools/files/write/mod.rs`
  - `execute_write` calls `freshness_rebase(&validated_path, expected_hash)` when `validated_path.exists()` and, on divergence, returns `BaseToolImpl::create_success_response(rebase)` — the silent no-op-as-success path. DELETE this whole `if validated_path.exists()` guard block.
  - DELETE `freshness_rebase` and the `HASH_LINE_PREFIX` const.
  - DELETE the `expected_hash` field from `WriteRequest` and the `expected_hash` entry from `WRITE_FILE_PARAMS`. (First confirm no caller/grammar still sends it; if one does, the param can be accepted-and-ignored for one release, but it MUST NOT influence behavior.)
- Docs: `crates/swissarmyhammer-tools/src/mcp/tools/files/write/description.md` — remove the entire "Read-before-write freshness guard" section and the divergence-return note under "Returns".
- Design origin: `ideas/file-edit-tools.md` — the "**`write files` — read-before-write guard.**" note must be rewritten to state that full-file write is unguarded (no hash check); the guard idea applies to `edit`, not `write`.

Note: the guard is **purely lost-update protection**. It is NOT required for the closed-write-surface / inline-diagnostics goal (`doc/src/concepts/closed-write-surface.md`) — those flow from the write going through the instrumented tool at all, regardless of any hash check. So removing it does not weaken diagnostics. A successful overwrite still carries the normal mutation envelope (`tagged_content` + `mutated_paths` + `bytes_written`).

## Tests to update / add (`write/mod.rs` test module)

- `test_write_overwrite_existing_file` — overwrite an existing file with NO `expected_hash`; assert new content on disk + mutation envelope.
- DELETE the guard tests entirely (the guarded behavior is gone): `test_write_existing_file_without_hash_does_not_clobber`, `test_write_existing_file_with_stale_hash_does_not_clobber`, `test_write_existing_file_with_matching_hash_succeeds`, `guard_divergence_write_has_no_mutation_envelope`, and the `create_test_arguments_with_hash` helper if now unused.
- Keep: new-file write, parent-dir creation, size limit, readonly-fails, atomic-write + cleanup, unicode, special chars, response-format / envelope round-trip (`anchor_from_write_envelope_resolves_in_edit`).

## Acceptance criteria

- `write files` on an existing path with no `expected_hash` overwrites the file and returns `is_error: false` with a mutation envelope (`bytes_written` > 0, `mutated_paths` set).
- No hash is read or compared anywhere in the write path; `expected_hash` no longer affects behavior.
- No code path returns the file's current content as a "success" without having written.
- `description.md` + `ideas/file-edit-tools.md` updated; no dangling references to `expected_hash` / freshness guard / re-base for the write op.
- `cargo test -p swissarmyhammer-tools` green.