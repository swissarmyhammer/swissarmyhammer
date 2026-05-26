---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
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
- [ ] `swissarmyhammer-agents` exposes the AGENT.md serializer; CLI calls it.
- [ ] No AGENT.md frontmatter knowledge remains in `apps/swissarmyhammer-cli/`.
- [ ] Roundtrip test exists and passes.
- [ ] `cargo build` and `cargo test -p swissarmyhammer-agents -p swissarmyhammer-cli` green.

## Tests
- [ ] New `swissarmyhammer-agents` roundtrip test (parse + serialize + assert equal up to field-presence rules).
- [ ] Existing agent-deployment integration tests continue to pass.

## Workflow
- Use `/tdd` — write the roundtrip test in `swissarmyhammer-agents` first. #init-doctor