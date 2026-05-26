---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8980
title: Move `format_agent_md` to swissarmyhammer-agents (agent format knowledge belongs there)
---
## What

`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs::format_agent_md` reconstructs an AGENT.md file (YAML frontmatter + rendered body) from a `swissarmyhammer_agents::Agent`. The CLI knows about every Agent field, optional ordering, and the AGENT.md serialization rules. That is agent format knowledge — and it should live next to the `Agent` struct in the `swissarmyhammer-agents` crate, not in a CLI install module.

Move and re-shape:

- Add an `Agent::to_agent_md(&self, rendered_body: &str) -> String` method (or a free function `swissarmyhammer_agents::format::agent_md(&Agent, body) -> String`) in `crates/swissarmyhammer-agents/src/…`. Same content shape as today's `format_agent_md`.
- The CLI `deploy_single_agent` (`components/mod.rs`) calls the new API: render body via `PromptLibrary`, then `agent.to_agent_md(&rendered)`.
- Add a roundtrip test in `swissarmyhammer-agents`: parse a sample AGENT.md → `Agent`, serialize back → assert the round-trip is stable.
- Delete `format_agent_md` from the CLI.

## Acceptance Criteria
- [x] `swissarmyhammer-agents` exposes the AGENT.md serializer; CLI calls it.
- [x] No AGENT.md frontmatter knowledge remains in `apps/swissarmyhammer-cli/`.
- [x] Roundtrip test exists and passes.
- [x] `cargo build` and `cargo test -p swissarmyhammer-agents -p swissarmyhammer-cli` green.

## Tests
- [x] New `swissarmyhammer-agents` roundtrip test (parse + serialize + assert equal up to field-presence rules).
- [x] Existing agent-deployment integration tests continue to pass.

## Workflow
- Use `/tdd` — write the roundtrip test in `swissarmyhammer-agents` first. #init-doctor

## Review Findings (2026-05-26 13:15)

### Nits
- [x] `crates/swissarmyhammer-agents/src/format.rs:9-11` — The module docstring claims "builtin agents do not carry a skills frontmatter list — skills are a runtime concept layered on top of the agent body." This is empirically false: `builtin/agents/reviewer/AGENT.md`, `builtin/agents/explore/AGENT.md`, `builtin/agents/implementer/AGENT.md`, and `builtin/agents/tester/AGENT.md` all have a `skills:` frontmatter block that the parser reads into `Agent::skills`. The serializer silently drops them on output, matching the old `format_agent_md` behavior — so this PR does not introduce the drop, only the false rationale for it. Either reword the docstring to state the truth ("skills are intentionally dropped during deploy because the runtime uses the parsed-in-memory list, not the on-disk AGENT.md"), or extend the serializer to emit `skills:` when non-empty and add a roundtrip test that covers it. The choice depends on what `mirdan::install::deploy_agent_to_agents` actually consumes downstream — if it never re-parses the materialized file for skills, dropping is fine and the docstring just needs a factual fix.

### Resolution (2026-05-26)
Reworded the module docstring in `crates/swissarmyhammer-agents/src/format.rs` to state the truth: several builtins do carry a `skills:` frontmatter, but `mirdan::install::deploy_agent_to_agents` only copies/symlinks the materialized file (verified at `crates/mirdan/src/install.rs:673-731` — no re-parse of the on-disk AGENT.md). Runtime consumers use the parsed in-memory `Agent` with `skills` intact, so dropping the field on output is correct; only the rationale needed fixing.