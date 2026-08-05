---
assignees:
- claude-code
position_column: todo
position_ordinal: fb80
title: AgentDef.instructions_path and its accessors have no production consumer after the Preamble removal
---
The Preamble component removal (^mawfv02) deleted the only production consumers of the agent instructions-file data:

- `mirdan::status::component_path` used `agent_global_instructions_file` / `agent_project_instructions_file` for `Component::Preamble`.
- `mirdan::install::apply_profile_preamble` used the same two accessors.

Both call sites are gone. What remains, with tests as the only callers:

- `AgentDef.instructions_path` and `AgentDef.global_instructions_path` (`crates/mirdan/src/agents.rs`)
- `agent_project_instructions_file` and `agent_global_instructions_file` (`crates/mirdan/src/agents.rs`)
- the `instructions_path` / `global_instructions_path` keys in `crates/mirdan/src/agents_default.yaml` for `claude-code`, `copilot`, and `codex`
- the same keys in the test fixtures `crates/mirdan/src/test_support.rs` and the inline YAML in `crates/mirdan/src/install.rs`

## Decide one of two

1. **Delete them.** Nothing reads the instructions file any more, so the field is a leftover of the removed preamble feature. `AgentDef` does not use `deny_unknown_fields`, so a user `~/.mirdan/agents.yaml` that still carries `instructions_path:` keeps loading after the field goes away.
2. **Keep them** as the declared agent layout, and record why in the doc comment, so the next reader does not read them as dead code.

The removal card ^mawfv02 did not include this in its scope, so it was left alone. Choose one and act.

## Blast radius for option 1

`AgentDef` struct literals set both fields in `crates/mirdan/src/agents.rs`, `crates/mirdan/src/doctor.rs`, `crates/mirdan/src/strategy/mod.rs`, `crates/mirdan/src/status.rs`, and `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs`. Every one needs an edit.

## Acceptance

- `cargo nextest run -p mirdan -p swissarmyhammer-cli` green
- `cargo clippy --workspace --all-targets -- -D warnings` clean #tech-debt