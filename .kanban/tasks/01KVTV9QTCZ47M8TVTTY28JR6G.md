---
assignees:
- claude-code
depends_on:
- 01KVTV89QPMT2Z63H2KZ8BJ3M1
position_column: todo
position_ordinal: a480
project: file-edit-tools
title: read files — emit hashline tags + whole-file content hash
---
## What
Make `read files` emit hashline-tagged output by default so anchors are the model's path of least resistance. Edit `crates/swissarmyhammer-tools/src/mcp/tools/files/read/mod.rs` (the shared `execute_read` handler).

- Add a `format` param to `READ_FILE_PARAMS` using `ParamMeta::allowed_values(&["hashline", "plain"])`, default `hashline`. Parse it on `ReadRequest`.
- In **hashline** form, prefix each emitted line `N:HH|` using `swissarmyhammer_hashline::tag` — N is the **absolute** 1-based line number so anchors stay stable across `offset`/`limit` windows (tag the window with `start_line = offset`). In **plain** form, emit current behavior unchanged.
- Leave the binary/base64 path **untagged** regardless of `format`.
- Emit a whole-file content hash (a stable digest of full file bytes) in the read result envelope so `write files`/`edit files` can use it as a freshness token for whole-file staleness. Surface it as a small trailing/leading metadata line or structured field (pick one and document it in `description.md`).
- Add `swissarmyhammer-hashline` to `swissarmyhammer-tools` `Cargo.toml` deps.

## Acceptance Criteria
- [ ] Default read output is hashline-tagged: each text line begins `N:HH|`, N absolute even with `offset` set.
- [ ] `format: "plain"` returns the pre-existing untagged output.
- [ ] Binary files return base64 untagged in both formats.
- [ ] The read result exposes a whole-file content hash usable as a freshness token.
- [ ] `offset`/`limit` still work and the tagged N matches the true file line number.

## Tests
- [ ] Unit tests in `read/mod.rs`: default hashline tagging; `plain` opt-out; absolute N under `offset`; binary stays untagged; content hash present and stable across identical reads.
- [ ] Update existing read tests that assert raw line content to tolerate/expect the tag prefix.
- [ ] `cargo test -p swissarmyhammer-tools read::` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.