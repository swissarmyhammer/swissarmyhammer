---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzhv43f9jwaewz8ea2me7vcm
  text: |-
    ### Delivered by `^n0680p8` — this card's work is done

    `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` was split under `^n0680p8`, where the same over-cap condition came back as a review finding on that card's own change. It went from 4890 lines / 187624 bytes to 337 lines / 14297 bytes, across the seams this card named — the op dispatch stayed in `mod.rs`, `index_discovered_files_async` went to `indexing.rs`, the per-op executors went to `execute.rs`, `status.rs` and `lsp_ops.rs`, and the inline test module went to `tests/` as five files.

    The public surface of the tool is unchanged: `mod.rs` re-exports `index_discovered_files_async`, every op struct, and the `LSP_SUPERVISOR` / session / `open_workspace` items the other consumers import.

    Acceptance, both halves: the largest new file's deterministic render cost is 48313 bytes against the 262144-byte cap (the old file's was 295204, over the cap before any probe evidence), and `cargo nextest run --workspace` is 13906 passed / 0 failed / 0 skipped with `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean.

    Nothing is left to do here. Close it.
  timestamp: 2026-08-08T23:25:24.841579+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffd080
title: split code_context/mod.rs — over the review per-file cap, 9 validators cannot review it
---
`crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` renders at 293953 bytes, over the 262144-byte per-file review cap. Every review that touches the file skips it and reports: "not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (split the file)". Raised as a CONFIRMED finding on the ^adf0d7h review (2026-08-08).

Split the file into modules that each fit the cap. Keep the public surface of the `code_context` tool unchanged. Candidate seams: the op dispatch, the indexing entry points (`index_discovered_files_async`), the per-op executors, and the inline test module.

Acceptance: a review that touches any of the new files reviews them (no per-file cap skip), and the workspace stays green.

#tool-validators