---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
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
- [ ] The preamble / MCP / deny-Bash detection predicates exist once (in `mirdan::status`) and are consumed by both the status feature and the sah-cli install layer.
- [ ] No behavior change to `mirdan status`, `mirdan doctor`, or `sah init`/`sah doctor` install detection.
- [ ] `cargo build` and `cargo test` are green for both `-p mirdan` and `-p swissarmyhammer-cli`.

## Tests
- [ ] A test asserting the shared detector agrees with what the install layer writes (write via install, detect via `mirdan::status`, expect Installed).
