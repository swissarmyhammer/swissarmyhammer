---
assignees:
- claude-code
position_column: todo
position_ordinal: ffa980
title: split code_context/mod.rs — over the review per-file cap, 9 validators cannot review it
---
`crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` renders at 293953 bytes, over the 262144-byte per-file review cap. Every review that touches the file skips it and reports: "not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (split the file)". Raised as a CONFIRMED finding on the ^adf0d7h review (2026-08-08).

Split the file into modules that each fit the cap. Keep the public surface of the `code_context` tool unchanged. Candidate seams: the op dispatch, the indexing entry points (`index_discovered_files_async`), the per-op executors, and the inline test module.

Acceptance: a review that touches any of the new files reviews them (no per-file cap skip), and the workspace stays green.

#tool-validators