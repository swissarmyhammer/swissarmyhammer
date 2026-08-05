---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz7hxg1peypj6p7fe3avk7sa
  text: |-
    Research done. Confirmed the decision gate before deleting: `rg -n 'preamble: true' --type rust` returns zero hits (exit 1). No live Rust code turns the preamble on, so option 1 (full removal) proceeded.

    Removed:
    - `Component::Preamble`, its label arm, its `component_path` arm, its `detect_component` arm. `Component::all()` is now `[Component; 4]`.
    - `PREAMBLE_MARKER`, `preamble_present`, `preamble_present_in` (`crates/mirdan/src/status.rs`).
    - `apply_profile_preamble`, `PreambleOutcome` (enum + impl), `ensure_preamble`, `remove_preamble`, and both `if profile.preamble` dispatch blocks in `init_profile`/`deinit_profile` (`crates/mirdan/src/install.rs`).
    - The `Profile.preamble` field and every `preamble: false` initializer in the five CLI profiles plus the two mirdan test profiles.
    - The now-unused `use crate::{settings, status}` import of `status` in `install.rs`.

    Tests retargeted rather than dropped, so the behaviour they covered stays covered:
    - `test_preamble_missing_then_installed` deleted — it tested only the deleted detector.
    - `codex_full_stack` now proves the same "the row genuinely participates in detection" property through `Component::Skills` (write a file into the user-scope skill dir, watch the cell flip Missing -> Installed).
    - The four `statuses_to_checks` scope-pair tests now compose their synthetic rows from `Component::Permissions` with settings.json paths.
    - `test_check_install_stack_user_scope_rows` (sah doctor) now deploys `~/.claude/skills/a-skill` instead of writing the CLAUDE.md marker; it still asserts installed-user -> Ok, missing-user-permissions -> Warning, and the project-row demotion.
    - Component-count assertions moved 5 -> 4 and 10 -> 8.

    Doc comments that described the removed behaviour were rewritten in `cli.rs`, `agents.rs`, `doctor/mod.rs`, `doctor/checks.rs`, `install/{init,deinit,mod,components/mod}.rs`, `commands/registry.rs`, `kanban-app/state.rs`, `swissarmyhammer-tools/.../kanban/mod.rs`, and the generated `doc/src/reference/mirdan-cli.md`.

    Discovery: `AgentDef.instructions_path` / `global_instructions_path` and the two `agent_*_instructions_file` accessors now have no production consumer — only their own tests. That is outside this card's stated scope, so it was left alone and filed as its own card.
  timestamp: 2026-08-04T23:32:06.838487+00:00
- actor: claude-code
  id: 01kz7qge491ssfw7dp6mkcmsr0
  text: |-
    Self-review ran four passes (`review working`). Every finding on code this change wrote or rewrote is fixed:

    - `Component::all()`'s `4` and `STATUS_SCOPES`'s `2` became `COMPONENT_COUNT` / `STATUS_SCOPE_COUNT`, and the three count assertions in the status tests now read the constant instead of a literal.
    - `check_all` and `check_all_doctored` share one `check_selected` sweep; they differ only by the agent predicate.
    - `run_status(bool, bool)` became `run_status(StatusVisibility, StatusFormat)`, with `from_all_flag` / `from_json_flag` keeping `dispatch` under the complexity gate.
    - `mcp_server_installed`'s `Vec::with_capacity(4)` became `MCP_SERVERS_KEY_CAPACITY`.
    - `detect_agents` split into `detect_agent` / `detection_hit` / `first_probed_dir`, under the complexity gate. It probes once — `find_map`'s result decides both `detected` and the detail.
    - `init::install` and `deinit::uninstall` collapsed onto one `run_lifecycle(Direction, ..)` in `install/mod.rs`; `Direction::pick` is the single branch behind the three wording accessors.
    - `Profile::tool(server_name, skills)` landed in mirdan, next to `ProfileMcpServer::serve`. kanban, code-context, and shelltool now declare that pair instead of respelling the struct.
    - `check_claude_mcp_list` takes `Option<&Path>`, and its duplicated `display_path`/`command_path` locals collapsed to one.
    - Named constants for the PATH-hint limit, the ProjectStructure priority, the DJB2 seed and multiplier, and the JPEG start-of-image marker; doc comments added to `Commands`, `NewKind`, and `set_text`.

    Findings dropped under the review skill's blanket rule against refactoring tests that already existed: the drag-session and MCP-shutdown timing literals and the board-count fixture in `kanban-app/src/state.rs`'s `mod tests`, and the minimum-agent-count assertion in `mirdan/src/agents.rs`'s `mod tests`.

    Findings NOT actioned, all pre-existing defects in files this card touched only to delete a stale word. Each pass surfaced a fresh crop of them, so the loop was not converging on this card's subject:

    - `apps/kanban-app/src/state.rs:31` — `TauriClipboardProvider` should derive `Debug` and `Clone`.
    - `apps/kanban-app/src/state.rs:363` — `open` takes `PathBuf`; it should take `&Path`.
    - `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:323` — the project-detection depth `3` should be a named constant.
    - `crates/mirdan/src/agents.rs:275,280,285,308,313` — the eight path accessors repeat one shape; the engine asks for a macro.
    - `crates/mirdan/src/dispatch.rs:15,43,189` — the `Err(e) => { eprintln!; 1 }` handler repeats.

    Also note: the duplication validator never read `crates/mirdan/src/install.rs` in any pass — at ~567 KB rendered it exceeds the agent prompt cap. That file is unreviewed for duplication.

    Verification: `cargo nextest run -p mirdan -p swissarmyhammer-cli -p kanban-cli -p code-context-cli -p shelltool-cli -p kanban-app` — 1433 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo fmt --all -- --check` clean.

    User-visible proof, built and run rather than inferred: `./target/debug/sah doctor | grep -ci preamble` prints `0`, and Claude Code now yields 8 rows (2 scopes x 4 components) where it previously yielded 10 with two permanent Preamble warnings. `./target/debug/mirdan doctor` likewise shows no Preamble row. `mirdan status` still renders its table (33 lines), `--all` still widens it (53 lines), and `--json` still emits JSON — the three paths through the new `StatusVisibility`/`StatusFormat` enums.
  timestamp: 2026-08-05T01:09:50.345685+00:00
- actor: claude-code
  id: 01kz7qghg30h1xmn6g1c96ajh8
  text: |-
    ### implement — changed
    - evidence: 20 source files. Removed `Component::Preamble`, `PREAMBLE_MARKER`, `preamble_present`, `preamble_present_in`, `apply_profile_preamble`, `PreambleOutcome`, `ensure_preamble`, `remove_preamble`, and `Profile.preamble`. `cargo nextest run -p mirdan -p swissarmyhammer-cli -p kanban-cli -p code-context-cli -p shelltool-cli -p kanban-app` — 1433 passed, 0 failed. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo fmt --all -- --check` clean. `sah doctor` and `mirdan doctor` print zero Preamble rows.
    - next: ready for `/review`. Five pre-existing findings in untouched parts of touched files are listed in the previous comment and were not actioned; a person decides whether they belong to this card.
  timestamp: 2026-08-05T01:09:53.795898+00:00
position_column: doing
position_ordinal: '8380'
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