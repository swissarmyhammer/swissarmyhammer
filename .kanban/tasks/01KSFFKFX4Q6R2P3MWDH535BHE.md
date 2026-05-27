---
assignees:
- claude-code
depends_on:
- 01KSFFJCKN0WV1C5D9MQ72VVWY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8480
title: 'mirdan: surface install-status via `mirdan status` + `mirdan doctor` (optional)'
---
## What

Make the agent-agnostic status capability directly usable from mirdan itself, so "mirdan can answer if we have these things installed" is true at the CLI, not only inside `sah doctor`.

- Add a `Status` variant to `mirdan`'s `Commands` enum (`crates/mirdan/src/cli.rs`) with an optional `--json` flag and optional `--all`/scope flags, wired through `crates/mirdan/src/dispatch.rs`.
- Implement `mirdan::status::run_status(json: bool)` (or place the runner in `status.rs`) that calls `check_all(&load_agents_config()?, &[InitScope::Project, InitScope::User])` and prints a table (reuse `crate::table`) with columns Agent / Scope / Component / State / Path, plus a JSON mode mirroring `run_agents`.
- Extend `crates/mirdan/src/doctor.rs`: add a `check_install_stack` to `MirdanDoctor::run_diagnostics` that adds a Check per non-NotApplicable `ComponentStatus` via `status::to_check`, so `mirdan doctor` reports the same stack `sah doctor` does.

This is additive and optional relative to the `sah doctor` work; keep it out of the critical path. Do it only after the status API card lands.

## Acceptance Criteria
- [x] `mirdan status` prints a table of (agent, scope, component, state, path) for detected agents; `mirdan status --json` prints structured JSON.
- [x] `mirdan doctor` includes install-stack checks sourced from `mirdan::status`.
- [x] `cargo build -p mirdan` is green.

## Tests
- [x] Unit test `run_status`/the JSON serializer against a synthetic config and assert the JSON shape (agents × scopes × components).
- [x] Add a `MirdanDoctor` test asserting the install-stack checks appear in `run_diagnostics` output.
- [x] `cargo test -p mirdan` runs green.

## Workflow
- Use `/tdd` — write the JSON-shape test first, then wire the command + doctor check. #init-doctor

## Implementation Notes
- `status.rs`: added `ComponentState::label()`, `status_json()` (pure JSON serializer, testable against a synthetic config), and `run_status(all, json)` over scopes `[Project, User]`. `--all` surfaces NotApplicable rows (hidden by default).
- `cli.rs`: added `Status { all, json }` variant.
- `dispatch.rs`: wired `Commands::Status` to `status::run_status`.
- `doctor.rs`: added `check_install_stack()` to `run_diagnostics`, emitting one Check per non-NotApplicable `ComponentStatus` via `status::to_check`. The doctor install-stack test asserts directly against `check_install_stack` (avoids the network call in `run_diagnostics::check_registry_reachable`).
- Verified end-to-end: `mirdan status`, `mirdan status --json`, and `mirdan doctor` all render the install stack. `cargo build/test/clippy/fmt -p mirdan` all green (279 tests pass).

## Review Findings (2026-05-25 11:18)

Verified locally: `cargo test -p mirdan` green (279 passed), `cargo clippy -p mirdan --all-targets` clean. Implementation is correct, well-documented, and all acceptance-criteria tests are present. No blockers. The items below are non-blocking quality improvements.

### Warnings
- [x] `crates/mirdan/src/status.rs:400-452` — The detection helpers `mcp_server_installed`, `preamble_present`, `permissions_present`, and `dir_non_empty` re-implement install-detection logic that already exists in the CLI install layer (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs` — e.g. the `CLAUDE_MD_PREAMBLE` first-non-empty-line check at ~line 1012, plus MCP/permissions probing). This card adds a second copy of that concept rather than extracting a shared detector. The two can now drift (e.g. the install layer writes the preamble; `status.rs` independently decides what "present" means). Consider extracting the detectors into `status.rs` and having the install components consume them (or vice versa) so install and status agree by construction. Out-of-scope to fully refactor here, but worth a follow-up card so the duplication does not calcify.
  - DEFERRED WITH TRACKING → new card `01KSFZ3EHCAGT2TNP3FWSKMDX6` ("Share install-detection predicates between mirdan::status and the sah-cli install layer". The duplicated detection lives in the *downstream* `swissarmyhammer-cli` crate (it depends one-way on `mirdan`), so unifying "by construction" requires making the `status.rs` detectors `pub` and rewiring `apps/swissarmyhammer-cli/.../install/components/mod.rs` — a cross-crate refactor outside the mirdan-only scope of this card. Not forced here per task guidance; captured in the follow-up so the duplication does not calcify.

### Nits
- [x] `crates/mirdan/src/doctor.rs:1-8` — Module doc comment lists the checks (1 PATH, 2 Agents, 3 AVP, 4 Registry, 5 Credentials) but omits the new install-stack check that `run_diagnostics` now runs at position 4 (before Registry). Update the list so the doc matches the actual check order.
  - FIXED: doc list now reads 1 PATH, 2 Agents, 3 AVP, 4 Install stack, 5 Registry, 6 Credentials — matching `run_diagnostics`.
- [x] `crates/mirdan/src/status.rs:392-413` — The `mcp_server_installed` doc comment claims it probes "both the default key and any value present" / the agent's configured key, but the implementation only probes the hardcoded `["mcpServers", "servers"]` and never consults the `AgentDef`'s `servers_key`. Correct for Claude Code, but the comment overstates generality. Either tighten the comment to say it probes the two common keys, or actually consult `servers_key` for non-default agents.
  - FIXED: tightened the doc comment to state it probes the two common keys (`mcpServers`, `servers`) and explicitly does not consult the agent's `servers_key`.
- [x] `crates/mirdan/src/status.rs:351` — In the `--json` branch, `visible` (a `Vec<&ComponentStatus>`) is cloned into an owned `Vec<ComponentStatus>` solely to satisfy `status_json(&[ComponentStatus])`. Negligible on this cold CLI path, but `status_json` could borrow (e.g. accept `&[&ComponentStatus]` or an iterator) to avoid the clone. Optional.
  - FIXED: `status_json` now takes `impl IntoIterator<Item = &ComponentStatus>`; the `--json` branch passes `visible.iter().copied()` with no clone. Existing tests pass `&[ComponentStatus]`/`&Vec<ComponentStatus>` slices unchanged (both satisfy the bound).

## Review Findings Resolution (2026-05-25)
All three nits fixed in-crate; the Warning deferred to tracking card `01KSFZ3EHCAGT2TNP3FWSKMDX6` per task guidance (cross-crate refactor out of scope). `cargo build/test/clippy -p mirdan` all green.