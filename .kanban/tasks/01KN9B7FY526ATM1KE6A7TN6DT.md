---
assignees:
- claude-code
position_column: todo
position_ordinal: b280
title: 'llama-agent: preload skills from agent definition frontmatter'
---
## What

Agent AGENT.md files can declare `skills:` in their YAML frontmatter (e.g. `builtin/agents/tester/AGENT.md` has `skills: [test]`, `builtin/agents/implementer/AGENT.md` has `skills: [implement]`). Currently:

1. `swissarmyhammer-agents` **ignores** the `skills:` field — `AgentFrontmatter` in `swissarmyhammer-agents/src/agent_loader.rs:11` has no `skills` field, and `Agent` in `swissarmyhammer-agents/src/agent.rs:72` has no `skills` field.
2. `llama-agent` builds agent system prompts in `resolve_mode_from_agent()` at `llama-agent/src/acp/config.rs:400` by rendering agent instructions through the prompt library, but never resolves or appends preloaded skill content.
3. `llama-agent` already has a `SkillLibrary` available via `CommandRegistry::with_skills()` at `llama-agent/src/acp/commands.rs:27`.

The work is to parse `skills:` from agent frontmatter, store them on the `Agent` struct, and have `llama-agent` resolve + append those skill instructions to the agent's system prompt when building mode system prompts.

### Files to modify

- `swissarmyhammer-agents/src/agent_loader.rs` — add `skills: Vec<String>` to `AgentFrontmatter`, pass through to `Agent`
- `swissarmyhammer-agents/src/agent.rs` — add `pub skills: Vec<String>` to `Agent` struct
- `llama-agent/src/acp/config.rs` — in `resolve_mode_from_agent()`, after rendering agent instructions, look up each skill in a `SkillLibrary`, render it, and append to the system prompt. The `SkillLibrary` needs to be threaded into this function (currently not available there — only `AgentLibrary` and `PromptLibrary` are passed).

## Acceptance Criteria

- [ ] `Agent` struct has a `skills: Vec&lt;String&gt;` field populated from AGENT.md frontmatter
- [ ] `tester` agent's parsed `Agent` has `skills: vec!["test"]`
- [ ] `implementer` agent's parsed `Agent` has `skills: vec!["implement"]`
- [ ] When llama-agent builds a mode system prompt for an agent with `skills: [test]`, the rendered system prompt includes the test skill's full instructions appended after the agent's own instructions
- [ ] Agents without `skills:` in frontmatter continue to work unchanged (empty vec, no appended content)

## Tests

- [ ] `swissarmyhammer-agents/src/agent_loader.rs` — unit test: parse AGENT.md with `skills: [test, implement]` frontmatter, verify `agent.skills == vec!["test", "implement"]`
- [ ] `swissarmyhammer-agents/src/agent_loader.rs` — unit test: parse AGENT.md without `skills:`, verify `agent.skills` is empty
- [ ] `llama-agent/src/acp/config.rs` — unit test: `resolve_mode_from_agent` with agent that has skills, verify system prompt contains skill instructions
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-agents) | rdeps(llama-agent)'` — all pass