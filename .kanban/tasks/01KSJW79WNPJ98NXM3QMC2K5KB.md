---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9180
title: 'Fix CI: `Agent::to_agent_md` must serialize `skills:` field (dropping it breaks resolver overrides)'
---
## What

`Agent::to_agent_md` in `crates/swissarmyhammer-agents/src/format.rs` deliberately omits the `skills:` frontmatter field on output (a carryover from the CLI's original `format_agent_md`, which I moved in card 01KSJ49P1C8K07C888GXH1KDZD with a docstring that explained the omission was intentional because deploy doesn't re-parse).

That reasoning is wrong for sah's own agent store. The CLI's deploy pipeline writes the serialized AGENT.md into the mirdan agent store (`~/.agents/<name>/AGENT.md` for User scope, `.agents/<name>/AGENT.md` for Project/Local). `swissarmyhammer_agents::AgentResolver` then loads agents with precedence `builtin < user < local` — so the deployed (skills-less) file overrides the embedded builtin (which has `skills: [test]` etc.). After `sah init`, `tester` is loaded with `skills: []` and runtime resolution fails.

Concrete symptom: CI test `llama-agent::acp::config::tests::test_resolve_mode_from_agent_appends_skill_instructions` panics with "system prompt should contain test skill instructions" because the in-memory `tester` agent's `skills` vec is empty after the resolver loads the override. Reproduces locally if `~/.agents/tester/AGENT.md` exists from a previous `sah init user`.

Fix:

1. **Serialize `skills` in `Agent::to_agent_md`** — append a `skills:\n  - <name>\n  - <name>` block when `!self.skills.is_empty()`. Keep the field omitted when the vec is empty (consistent with the field's `#[serde(default)]` shape). Update the module docstring to explain that skills are preserved precisely because the same serializer feeds the mirdan store and the resolver reads back from it.
2. **Strengthen the roundtrip test** — extend `crates/swissarmyhammer-agents/src/format.rs::tests::test_agent_md_roundtrip` (or add a sibling) to assert that an Agent with `skills: vec!["test", "implement"]` round-trips parse → serialize → parse with the `skills` vec preserved.
3. **No behavior change for coding-tool subagent dirs** — Claude Code (and the other tools' `agents/` dirs) is permissive about extra YAML frontmatter keys; preserving `skills:` will not break their consumers. Verify by re-reading the deployed file with `mirdan::install::deploy_agent_to_agents` semantics in mind — it only copies/symlinks, no re-parsing.

## Implementation Notes

- Found a second, latent bug in the llama-agent test: it compared the raw (un-rendered) skill instructions snippet against the rendered system_prompt. The first ~50 chars of the test skill are `{% include "_partials/coding-standards" %}` which the resolver expands before concatenation, so the literal include syntax never appears in the rendered output. Fixed the test to render the skill snippet through `prompt_library.render_text` before comparing — same intent (verify skill content is appended), correct assertion.

## Acceptance Criteria
- [x] `Agent::to_agent_md` emits `skills:\n  - <name>` lines whenever `!self.skills.is_empty()`.
- [x] Roundtrip test covers an agent with a non-empty `skills` vec.
- [x] `cargo test -p llama-agent acp::config::tests::test_resolve_mode_from_agent_appends_skill_instructions` passes locally after the fix **and** after removing the stale local override (or after re-deploying with the fixed serializer).
- [x] `cargo test -p swissarmyhammer-agents` green.
- [x] `cargo clippy -p swissarmyhammer-agents -p swissarmyhammer-cli --all-targets -- -D warnings` clean.

## Tests
- [x] Roundtrip test with non-empty `skills`.
- [x] Targeted llama-agent test passes (after the local `~/.agents/tester/AGENT.md` is regenerated with the new serializer or removed so the builtin wins).

## Workflow
- Read `crates/swissarmyhammer-agents/src/format.rs::Agent::to_agent_md` and the existing roundtrip test; mirror the YAML emission pattern used by `tools:` and `disallowed-tools:`. #ci-fix