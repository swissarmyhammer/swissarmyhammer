---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffac80
title: Share install-detection predicates between mirdan::status and the sah-cli install layer
---
## What

`crates/mirdan/src/status.rs` and the CLI install layer (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`) independently implement the same install-detection concepts. The two can drift: the install layer *writes* the artifacts (preamble, MCP server entry, Bash deny rule) while `status.rs` *detects* them with its own predicates, and nothing guarantees they agree.

Concretely, the duplicated detection predicates are:
- Preamble present — `status::preamble_present` (first-non-empty-line contains `PREAMBLE_MARKER`) vs. the inline `content.lines().find(...)` checks in the install layer's `ensure`/`merge` functions plus the `#[cfg(test)]` `preamble_file_has_preamble` helper.
- MCP server installed — `status::mcp_server_installed` (probes `mcpServers`/`servers` for a `sah` entry with a `sah`-ish command) vs. the install layer's register/unregister + `cleanup_empty_mcp_servers` logic.
- Permissions/deny-Bash present — `status::permissions_present` (`permissions.deny` contains `"Bash"`) vs. `DenyBash` / `settings::merge_deny_bash`.

Note: the `PREAMBLE_MARKER` constant is *already* shared (`pub use mirdan::status::PREAMBLE_MARKER as CLAUDE_MD_PREAMBLE`), and MCP registration already routes through `mirdan::mcp_config` with the agent's `servers_key`. So the precedent for sharing through mirdan exists — only the detection predicates remain duplicated.

## Why not done in 01KSFFKFX4Q6R2P3MWDH535BHE

Surfaced as a non-blocking Warning on that card. Deferred because: `swissarmyhammer-cli` depends on `mirdan` (one-way), so the install layer that re-implements detection lives in the *downstream* crate. Unifying "by construction" means making the `status.rs` detectors `pub` and rewiring `apps/swissarmyhammer-cli/.../install/components/mod.rs` (production ensure/merge paths and its test helper) to consume them — a cross-crate change outside the mirdan-only scope of that card.

## Proposed approach

- Promote the three detectors in `crates/mirdan/src/status.rs` to `pub` (or a small `pub` detection sub-module) with docs that make them the single source of truth for "is component X installed at this path".
- In `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`, replace the inline / `#[cfg(test)]` detection checks with calls to the mirdan detectors so install and status agree by construction. While the MCP detector currently probes the two common server keys, prefer threading the agent's `servers_key` if/when the install layer's per-agent context makes that natural.

## Acceptance Criteria
- [x] The preamble / MCP / deny-Bash detection predicates exist once (in `mirdan::status`) and are consumed by both the status feature and the sah-cli install layer.
- [x] No behavior change to `mirdan status`, `mirdan doctor`, or `sah init`/`sah doctor` install detection.
- [x] `cargo build` and `cargo test` are green for both `-p mirdan` and `-p swissarmyhammer-cli`.

## Tests
- [x] A test asserting the shared detector agrees with what the install layer writes (write via install, detect via `mirdan::status`, expect Installed).

## Review Findings (2026-05-27 13:00)

### Nits
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:1277` — `preamble_file_has_preamble` now returns `Some(false)` (not `None`) when the file exists but `read_to_string` fails (e.g. permission denied), because that error is swallowed inside `mirdan::status::preamble_present`. Previously the helper used `read_to_string(path).ok()?` and returned `None`. Behavior on an unreadable existing file is now "missing preamble" rather than "not detectable". This is a tiny edge-case behavioral shift; unlikely to matter in practice but worth noting in the doc comment if the distinction is meant to be preserved. — Addressed by extending the doc comment on `preamble_file_has_preamble` to call out the `Some(false)` vs `None` shift for unreadable-but-existing files.
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:1292-1311` — `ensure_preamble` and `remove_preamble` now read the file twice on the "needs work" path: once inside `mirdan::status::preamble_present`, then again via `read_to_string` for the actual content manipulation. For CLAUDE.md this is negligible, but if a small helper in `mirdan::status` exposed both "is-present" and "content" in one call (or if `preamble_present` were paired with a `read_with_preamble_state` returning both), the duplication would go away. Optional cleanup. — Addressed by adding `pub fn preamble_present_in(content: &str) -> bool` in `mirdan::status`. `preamble_present(path)` now delegates to it, and the install layer reads the file once then checks via `preamble_present_in`, eliminating the double read.
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:1662-1678` — The agreement test `test_install_deny_bash_agrees_with_status_detector` constructs a full `mirdan::agents::AgentDef` literal with every field listed. If `AgentDef` ever gains a field this test fails to compile. A `..AgentDef::default()`-style spread (if available) or a small `synthetic_agent_def(settings_path)` helper would make the test more change-tolerant. Pure brittleness nit. — Addressed by introducing `synthetic_agent_def(global_settings: &Path) -> AgentDef` in the test module and rewriting the test to use it. New fields on `AgentDef` only need to be added once, in the helper.
- [ ] `.config/nextest.toml:43-54` — The new `swissarmyhammer-tools` code_context override is a legitimate and well-explained fix for embedding-load contention, but it is scope-creep relative to this task's stated subject ("Share install-detection predicates…"). Ideally it would have been a separate commit/task so the install-detection change has a focused diff. Not blocking; calling it out for hygiene. — Left unchecked: this is a process/hygiene complaint about how the prior commit was scoped, not an actionable code change. The reviewer themselves note the override is "legitimate and well-explained" — reverting it would break the test suite. There is nothing to flip; the box would only be honest if a separate commit/task were retroactively created, which is out of scope here.
