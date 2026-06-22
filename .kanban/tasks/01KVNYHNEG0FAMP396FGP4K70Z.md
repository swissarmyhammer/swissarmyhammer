---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvnyj3mxxtrsnw2ekmgt399c
  text: 'Picked up in doing. Implementation already drafted in the working tree this session: shared `deserialize_flexible_usize` helper in files/shared_utils.rs, applied to read offset/limit and grep context_lines, plus 5 tests. Driving through the full finish pipeline (implement verify → test → review → done → commit) to gate it properly.'
  timestamp: 2026-06-21T20:39:39.933555+00:00
- actor: claude-code
  id: 01kvnywqap0f8t0htr1ea3rc8e
  text: |-
    Verified the drafted string-encoded-numeric-args fix and moved to review.

    Acceptance criteria — all satisfied:
    1. read offset/limit accept int OR string-encoded int — ReadRequest.offset/limit annotated #[serde(default, deserialize_with = "...deserialize_flexible_usize")].
    2. grep context_lines same — GrepRequest.context_lines annotated identically.
    3. absent/null/empty -> None; non-numeric & negative rejected — handled in deserialize_flexible_usize (trims whitespace, empty/whitespace-only -> None, rejects non-numeric and negatives via usize::try_from).
    4. Coercion implemented ONCE — single deserialize_flexible_usize in shared_utils.rs referenced by all three fields; no per-field duplication.
    5. Regression + focused unit tests — test_read_with_string_offset_and_limit ("2"/"2"), 4 helper unit tests, plus a NEW test_grep_string_context_lines ("1") I added.

    Fix I made: the adversarial double-check (advisory gate) flagged that grep's string-coercion path had no end-to-end regression test — criterion 2/5 for grep was only covered transitively via the helper unit tests. Added test_grep_string_context_lines mirroring test_grep_context_lines_explicit but passing context_lines as string "1", asserting identical output. TDD-verified: with the deserialize_with annotation temporarily removed the new test fails with the exact original bug (invalid type: string "1", expected usize); restored, it passes. This pins the wiring on the actual GrepRequest struct.

    Test/clippy/fmt results (fresh):
    - cargo nextest run -p swissarmyhammer-tools -E 'test(files)': 205 tests run, 205 passed, 0 failed.
    - 6 targeted tests (4 helper unit + read regression + new grep regression): all pass.
    - cargo clippy -p swissarmyhammer-tools --lib -- -D warnings: clean, exit 0.
    - cargo fmt --check on the three touched files: clean (ran cargo fmt to apply minor reflow).
  timestamp: 2026-06-21T20:45:27.766548+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffcd80
title: files read/grep tools reject string-encoded numeric args (offset/limit/context_lines)
---
An agent reported: `files read_file: Failed to parse arguments: invalid type: string "60"/"40", expected usize`. Language models routinely stringify numeric tool arguments (sending `"60"` instead of `60`), but the MCP `files` tool typed `offset`/`limit`/`context_lines` as plain `Option<usize>`, which only accepts a JSON number. The result is a rejected call and a wasteful model retry.

#bug

Acceptance criteria:
- The `read file` op accepts `offset`/`limit` as either JSON integers or string-encoded integers.
- The `grep files` op accepts `context_lines` the same way.
- Absent / null / empty-string resolve to `None`; non-numeric and negative input are rejected.
- Coercion is implemented once via a shared serde helper (no per-field duplication).
- Regression test reproduces the exact `"2"`/`"2"` string scenario; focused unit tests cover the helper.
- `cargo test -p swissarmyhammer-tools --lib files::` and `cargo clippy -p swissarmyhammer-tools --lib` are green.

Fix location: crates/swissarmyhammer-tools/src/mcp/tools/files/ (shared_utils.rs helper, read/mod.rs, grep/mod.rs).