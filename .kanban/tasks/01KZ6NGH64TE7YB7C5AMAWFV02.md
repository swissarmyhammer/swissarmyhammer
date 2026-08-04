---
assignees:
- claude-code
position_column: todo
position_ordinal: f780
title: Doctor's Preamble check is stale — nothing installs the preamble anymore, so it always reports Missing
---
`sah doctor` / `mirdan doctor` unconditionally reports a `Missing` (Warning) row for the `Preamble` component on every doctor-enabled agent, at both project and user scope. This looks like a false positive: no install profile in the codebase writes the preamble anymore.

## Root cause

`Component::all()` in `crates/mirdan/src/status.rs:61-69` hardcodes 5 components checked for every agent: `Mcp`, `Skills`, `Agents`, `Preamble`, `Permissions`. `check_component` (line 170) checks `Component::Preamble` purely by reading the agent's instructions file (`CLAUDE.md`/`AGENTS.md`) for `PREAMBLE_MARKER` (`"MANDATORY: load the thoughtful skill if not already loaded."`, `status.rs:31`) — it has no awareness of any install `Profile` or whether preamble installation is even enabled.

Every real `Profile` in the codebase sets `preamble: false`:
- `apps/swissarmyhammer-cli/src/commands/profile.rs:42` — sah's own profile
- `apps/kanban-cli/src/commands/registry.rs:40`
- `apps/code-context-cli/src/commands/registry.rs:41` and `skill.rs:35`
- `apps/shelltool-cli/src/commands/registry.rs:41`
- `crates/mirdan/src/install.rs:5552`, `:6021` (defaults)

`grep -rn "preamble: true"` across the whole workspace returns zero hits in live Rust code — only in stale kanban task markdown from when the feature was first designed (`^t7a3z4f`). So no install path run by any current CLI ever writes `PREAMBLE_MARKER` into an agent's instructions file. `sah init` (and every other CLI's init) deliberately does not touch CLAUDE.md's content for this purpose anymore.

Given that, `check_component(Component::Preamble)` will report `Missing` for every agent, on every host, forever — `mirdan::install::ensure_preamble`/`remove_preamble` (`install.rs:1742-1808`) are dead code paths reachable only if some future profile sets `preamble: true`, and `apply_profile_preamble` (`install.rs:1657`) is gated on `profile.preamble` which is false everywhere.

## Why this matters

- `sah doctor` shows a permanent, unfixable-by-`sah init` Warning: `Claude Code · project · Preamble` (and `· user ·`) will never turn green by running the suggested fix (`sah init` / `sah init user`), because no profile writes it. This is a red herring that erodes trust in doctor output.
- Confirmed via reading `check_install_stack_with`/`state_of`/`check_component` in `crates/mirdan/src/status.rs` and `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs` — no `profile.preamble` gate anywhere in the doctor path, only in the installer.

## Fix

Pick one, per what the project intends for the preamble feature's future:

1. **Fully remove the Preamble component from doctor** (and likely from `Component` entirely, plus `ensure_preamble`/`remove_preamble`/`apply_profile_preamble`/`PREAMBLE_MARKER`/`preamble_present`/the `Profile.preamble` field) if the CLAUDE.md-preamble-injection feature itself is retired. This is the "we removed the feature" reading — do it thoroughly: dead code left behind (an unreachable installer branch plus a doctor check for it) is worse than removing it outright.
2. **Or**, if the intent is to keep the *capability* to install a preamble (some future profile might opt in) without checking for it when no profile wants it, make the doctor's Preamble row conditional — skip it (map to `NotApplicable`/omit) when no currently-relevant profile has `preamble: true`. This requires threading a profile-aware signal into `check_component`/`check_agent`, which doctor's current architecture doesn't have (it only has `AgentDef`, not `Profile`).

Given every live profile already sets `preamble: false` and grep finds zero remaining `preamble: true` in Rust code, treat this as a completed removal that missed cleaning up the doctor check and the dead installer code — option 1 is very likely correct. Confirm with whoever removed the CLAUDE.md-preamble-injection feature before deleting the installer code, since `ensure_preamble`/`remove_preamble` may still be intentionally kept for a future profile.

## Acceptance

- `sah doctor` and `mirdan doctor` no longer show a `Preamble` row that can never be resolved by running `sah init`/`sah init user`.
- If the component is removed: `Component::Preamble`, `PREAMBLE_MARKER`, `preamble_present`/`preamble_present_in`, `ensure_preamble`, `remove_preamble`, `apply_profile_preamble`, and the `Profile.preamble` field are all deleted, along with every test that exercises them (`crates/mirdan/src/status.rs` and `crates/mirdan/src/install.rs` test modules), and `Component::all()` drops to 4 entries (update the `1 agent × N scopes × 5 components` test assertions accordingly).
- `cargo nextest run -p mirdan -p swissarmyhammer-cli` green, `cargo clippy --workspace --all-targets -- -D warnings` clean. #bug