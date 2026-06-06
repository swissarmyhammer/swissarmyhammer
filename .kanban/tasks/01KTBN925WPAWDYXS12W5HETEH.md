---
assignees:
- claude-code
depends_on:
- 01KTBNFB7NPXNWKDK86T9A0M5C
- 01KTBNGHCH7B3J3DVF9CXPADJ1
- 01KTBNFMEAS2QQRH7N0RVDKC27
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffef80
project: local-review
title: Rename avp-common → swissarmyhammer-validators (the pluggable review engine crate)
---
## What
Rename the now-slimmed `avp-common` crate (loader + extracted ACP executor + validator types, after the hook machinery and avp-cli are gone) to **`swissarmyhammer-validators`**. This becomes the shared engine crate for the pluggable review system; the new `review` MCP tool and the engine stages all live here or depend on it.

- Rename `crates/avp-common/` → `crates/swissarmyhammer-validators/`; set `name = "swissarmyhammer-validators"` in its `Cargo.toml`.
- Update every dependent's `Cargo.toml` and `use` path: `agent-client-protocol-extras`, `apps/swissarmyhammer-cli`, the root `Cargo.toml` `[workspace.members]` + `[workspace.dependencies]`. (`apps/avp-cli` is already deleted.)
- Pick the canonical crate-internal module name (`validators`) and update re-exports so `swissarmyhammer_validators::ValidatorLoader`, `::Finding` (added in a later task), and the `execute_agents` primitive are the public surface.
- Expose a clean, hook-free engine API at the crate root: `load_rules()`, `match_rules(file_path)`, `execute_agents(...)`. This is the decoupled, standalone rule-matching surface (no hook/ACP-hook arguments) that downstream stages consume.
- Keep `builtin/validators/**` rule data discoverable by the renamed loader (the on-disk directory move to `~/.validators` / `./.validators` is a separate task).

## Acceptance Criteria
- [ ] `crates/swissarmyhammer-validators/` exists; no crate named `avp-common` remains; `cargo build --workspace` green.
- [ ] All dependents compile against the new crate name; `rg -n "avp_common|avp-common"` returns nothing outside historical docs.
- [ ] The crate root re-exports the hook-free engine API (`load_rules`/`match_rules`/`execute_agents`), callable with NO hook/ACP-hook arguments.
- [ ] `crates/swissarmyhammer-tools` can add `swissarmyhammer-validators` as a dependency and call `match_rules` in a smoke test.

## Tests
- [ ] `cargo build --workspace` and `cargo test --workspace` green after rename.
- [ ] A loader test (moved with the crate) proving builtin → user → project precedence + glob matching still resolve identically.
- [ ] Smoke test from `swissarmyhammer-tools`: depend on the crate, call `match_rules("foo.rs")`, assert the source-code ruleset matches.

## Workflow
- Mechanical rename + API-surface cleanup. No `/tdd`; rely on the moved loader/executor tests. Do the rename as one atomic change (git mv + sed the dependents) so the workspace never sits half-renamed. Depends on all three teardown tasks so the residue being renamed is already the clean engine.