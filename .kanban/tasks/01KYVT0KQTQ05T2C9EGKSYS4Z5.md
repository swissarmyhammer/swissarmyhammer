---
assignees:
- claude-code
position_column: todo
position_ordinal: bd80
title: Agent install strips unknown frontmatter (same defect as ^t7ebyn8, agent side)
---
Found while fixing ^t7ebyn8 (skill frontmatter round-trip). The AGENT.md deploy path has the identical defect, one crate over.

## Defect

`crates/swissarmyhammer-agents/src/format.rs::Agent::to_agent_md` rebuilds AGENT.md frontmatter from a closed struct:

- `AgentFrontmatter` (`crates/swissarmyhammer-agents/src/agent_loader.rs`) has no `#[serde(flatten)] extra`, so serde drops every unmodeled key with no error.
- `Agent` (`crates/swissarmyhammer-agents/src/agent.rs`) has no `extra` field, so a dropped key cannot come back.

`install_profile_agents` calls `to_agent_md` on every `sah init`, so any AGENT.md key the struct does not name is lost — exactly how the skill-scoped `hooks:` block was lost from `builtin/skills/finish/SKILL.md`.

Note: the ^t7ebyn8 card description asserted "Agents are not affected. They deploy verbatim as symlinks." That is wrong. `to_agent_md` is a real re-serialize path.

## Not yet biting

The eight builtin AGENT.md files use only modeled keys (`name`, `description`, `skills`, `model`, `disallowed-tools`, `tools`, `isolation`, `max-turns`, `background`), so there is no active loss today. A user AGENT.md carrying `hooks:` loses it silently.

## Second problem in the same function

`to_agent_md` concatenates YAML by hand:

```rust
content.push_str(&format!("tools: \"{}\"\n", tools));
```

No escaping. A description or tool name containing a quote, colon or newline emits invalid YAML. The skill path avoids this by serializing through `serde_yaml_ng::to_string`.

## Required change

1. Mirror ^t7ebyn8: `#[serde(flatten)] extra: BTreeMap<String, serde_yaml_ng::Value>` on `AgentFrontmatter`, carry `extra` on `Agent`, flatten it back out on write.
2. Replace the hand-concatenation with a `#[derive(Serialize)]` frontmatter struct rendered by `serde_yaml_ng::to_string`, as `swissarmyhammer-skills::deploy::format_skill_md` does.
3. Reuse `swissarmyhammer_skills::SAH_INTERNAL_FRONTMATTER_KEYS` if any agent key turns out to be SAH-internal; check rather than assume.

## Acceptance

- Round-trip test: parse an AGENT.md carrying `hooks` and other unmodeled keys, format it, assert every key survives. Prove RED first.
- Test that a description containing a quote and a colon round-trips as valid YAML.
- Cover the real `install_profile_agents` deploy path, not only the unit level. #bug #init