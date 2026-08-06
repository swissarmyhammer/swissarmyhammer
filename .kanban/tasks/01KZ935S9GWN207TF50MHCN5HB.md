---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzcf3d6vq172ac2pf45ep7xx
  text: |-
    Research done.

    Facts found:
    - The tool-rule health decision lives once in `crates/swissarmyhammer-validators/src/doctor.rs` (`check_tool_rule` / `check_presence` / `run_shell`). The planner `review/tool_rules.rs::plan_rule_by_health` calls it. The install lifecycle must reuse `check_presence` so "installed" can never mean two things.
    - `run_review` (`review/synthesize.rs`) calls `plan_tool_rules` in stage 2, after scope and before batching. That is where an install stage goes: install first, then plan — the planner re-runs the doctor check, so a tool installed by the lifecycle is picked up with no extra plumbing.
    - No builtin tool rule ships yet (`grep -rn "^tool:" builtin/validators/` is empty). Card ^b01gtzg ("missing-docs runners for Rust and Python") adds the first ones. So version pinning must land as a guard over the builtin stack, not as an edit to spec files that do not exist.
    - `sah init` applies `sah_profile()` (`apps/swissarmyhammer-cli/src/commands/profile.rs`) through `mirdan::install::init_profile`, plus the `Initializable` components in `commands/registry.rs::register_all`.

    Placement decision for the "register the runner tools so `sah init` / mirdan install can pre-install them" bullet:
    - `mirdan` does NOT depend on `swissarmyhammer-validators`, and it must not: mirdan is the shared installer for every tool CLI (kanban, code-context, shelltool), so that edge would link the whole review engine (claude-agent, rusqlite, model-embedding, git2) into all of them.
    - mirdan cannot read a tool rule's `tool.install.commands` without that edge, so a `Profile` field would be data mirdan can neither fill nor apply.
    - The registration therefore lands on the seam `apps/swissarmyhammer-cli/src/commands/install/components/` already documents for "concerns not expressible as profile data": a `ValidatorTools` component that `register_all` registers, so `sah init` pre-installs the runner tools alongside the profile. One implementation of the lifecycle, no new crate edges.
  timestamp: 2026-08-06T21:19:06.971296+00:00
- actor: claude-code
  id: 01kzcg3da2gcrkxp72veqabwt5
  text: |-
    Implementation landed.

    What was built:
    - `crates/swissarmyhammer-validators/src/review/tool_install.rs` (new) — the whole install lifecycle. `install_tool_commands` runs each `install.commands` entry in order and re-runs the doctor check after each; a command that exits 0 and leaves the check failing is NOT a success. `ensure_tool_installed` adds one bounded agent turn when every command failed, then re-runs the doctor check — the agent's answer decides nothing. `ToolInstallAgent` is the seam; `PoolInstallAgent` is the production impl over the review's shared `AgentPool` (one turn, the pool's token cap, idle window, and turn ceiling bound it).
    - `review/tool_rules.rs` — extracted `matched_tool_rules`, the ONE matching pass. `plan_tool_rules` plans from it and `install_missing_tools` installs from it, so the engine can never install a tool for a rule it will not run.
    - `review/synthesize.rs` — `run_review` stage 2a installs before planning. The planner re-runs the same doctor check, so an installed tool is planned healthy and a still-missing one falls back on its own; no extra plumbing.
    - `doctor.rs` — extracted `project_tool_rules`, the ONE selection pass over the loader. `check_review_engine_with` diagnoses those rules and `install_project_tool_rules` installs their tools, so doctor can never report a rule the installer skipped. `check_presence` is now `pub(crate)` so the lifecycle and doctor agree on "installed".
    - Version pinning: `install_command_pins_version` plus a guard test over the whole builtin stack. No builtin tool rule ships install commands yet, so the guard becomes a hard requirement the moment ^b01gtzg adds the first ones.
    - `sah init` pre-install: the `ValidatorTools` `Initializable` component (priority 60), registered in `commands/registry.rs`. It runs the deterministic half only — `sah init` never spends an agent turn. A tool it cannot install is a Warning naming the rule, never an Error. `deinit` removes nothing: these are shared developer tools.

    Discoveries worth keeping:
    - A Rust `\`-continued string literal strips the NEXT line's leading whitespace. A test fixture built that way silently flattened the YAML nesting, so `install:` parsed as a top-level rule key instead of a `tool` key and the install list vanished. `RuleFrontmatter` has no `deny_unknown_fields`, so the misplaced key was accepted in silence. Build rule fixtures with raw multi-line literals.
    - RED verified by mutation, not by assumption: making the loop trust the command exit code and making the agent's answer decide broke 6 of the 13 lifecycle tests, each for its own reason. Reverted afterwards.

    Real-pipeline coverage:
    - `the_pool_install_agent_runs_a_real_turn_whose_claim_still_loses_to_doctor` drives a real `AgentPool` over a real ACP connection to a scripted agent that claims success; the doctor check still refuses it.
    - The two acceptance tests live in `tool_rules.rs` and run install → plan together over a real work-list and the real planner.
    - The `sah init` component tests build the rule through the real validator parser, not a hand-made struct.

    Tests: `swissarmyhammer-validators --lib` 449 passed, `swissarmyhammer-cli --lib` 295 passed, `swissarmyhammer-tools` review smoke + progress tests passed, `cargo check --workspace --all-targets` clean, clippy clean, fmt clean.
  timestamp: 2026-08-06T21:36:35.650970+00:00
- actor: claude-code
  id: 01kzcg3nw6jvk1phfme3dsnsxx
  text: |-
    ### implement — changed
    - evidence: 10 files — crates/swissarmyhammer-validators/src/review/tool_install.rs (new), review/tool_rules.rs, review/synthesize.rs, review/scope.rs, review/mod.rs, doctor.rs, apps/swissarmyhammer-cli/src/commands/install/components/mod.rs, apps/swissarmyhammer-cli/src/commands/registry.rs, builtin/validators/README.md, ARCHITECTURE.md. Tests: validators --lib 449 passed, cli --lib 295 passed, workspace check + clippy + fmt clean.
    - next: /review
  timestamp: 2026-08-06T21:36:44.422267+00:00
- actor: claude-code
  id: 01kzcgxr6cwzf6gjthvmf41tka
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13650 tests run, 13650 passed, 0 failed, 0 skipped; `cargo fmt --check` clean; `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.
    - fix applied: `crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs` — `WARM_UP_MAX_ATTEMPTS` raised from 20 to 120 (10s of poll budget to 60s). Two tests (`follower_multi_step_rename_gets_real_leader_edits_under_one_lock`, `follower_request_with_document_gets_real_definition_without_leader_preopen`) failed on the first full-workspace run: each drives a real `rust-analyzer` and polls until it warms up, but the old ~13s total budget (3s initial settle + 20×500ms poll) was tuned for an idle machine. Under full `--workspace` parallelism (13650 tests across the whole suite contending CPU) the analyzer stayed cold past that window even though the `lsp-ipc-serial` nextest test-group already caps this crate's LSP tests to one real analyzer at a time — the contention is from every other package's tests sharing the CPU, not a second analyzer. The sibling test `ra_pull_readiness.rs` in the same crate already uses a 60s deadline for the identical "real rust-analyzer, cold under CI load" scenario, so the fix matches that existing, already load-tolerant precedent. Verified: isolated run passed before and after; the full `cargo nextest run --workspace` after the fix is 13650/13650 green with 0 failures.
    - next: none.
  timestamp: 2026-08-06T21:50:58.764397+00:00
depends_on:
- 01KZ935GJX1YS2EAD7C2HK89AJ
position_column: doing
position_ordinal: '8480'
title: Runner install lifecycle with LLM installer fallback
---
Install missing runner tools through the unified mirdan tool-install lifecycle.

The contract is the Install lifecycle section of `builtin/validators/README.md`.

Work:
- On a missing tool: try each entry in `install.commands` in order. Re-run the doctor check after each try.
- Pin tool versions in the builtin runner specs. An unpinned tool can change rules and break the gate.
- If every command fails, spawn a bounded install agent. Inputs: the runner spec, the platform, the error output. Goal: make `doctor.check_command` pass. Doctor confirms the result. The agent cannot assert success.
- If the tool is still missing, the review falls back to the prompt rule and doctor keeps a warning. A missing tool never blocks a review.
- Register the runner tools in the mirdan Profile manifest so `sah init` / mirdan install can pre-install them.

Acceptance:
- With the tool absent and a working install command, the review installs it and runs the runner.
- With all installs failing, the review completes on the prompt fallback and doctor shows the warning.

#tool-validators