---
assignees:
- claude-code
position_column: todo
position_ordinal: b680
title: Skill install strips unknown frontmatter (ralph Stop hook is lost)
---
Skill install is a lossy parse-then-rebuild. It keeps an allowlist of 9 frontmatter fields and discards every other key without a warning.

## Symptom

`builtin/skills/finish/SKILL.md:12-16` declares a skill-scoped Stop hook:

```yaml
hooks:
  Stop:
    - hooks:
        - type: command
          command: "sah tool ralph ralph check --"
```

The deployed file has no `hooks` key:

```
$ awk '/^---$/{n++; if(n==2) exit} {print}' ~/.skills/finish/SKILL.md
---
name: finish
description: ...
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` and `ralph` MCP tools plus a Stop-hook-capable harness.
metadata:
  author: swissarmyhammer
  version: 0.17.0
```

`~/.claude/skills/finish` is a symlink to `~/.skills/finish`, so Claude Code reads the stripped file. Result: `/finish` calls `set ralph`, but no Stop hook exists to block the stop. The loop dies between iterations.

`hooks` is an official Claude Code skill frontmatter field. See https://code.claude.com/docs/en/skills#frontmatter-reference and https://code.claude.com/docs/en/hooks#hooks-in-skills-and-agents. Skill-scoped hooks are active only while the skill runs, which is the correct behavior for `/finish`.

## Cause

Both sides of the round-trip use a closed struct:

- `crates/swissarmyhammer-skills/src/skill_loader.rs:11-28` — `SkillFrontmatter` names 9 fields. Serde discards unknown keys. There is no `deny_unknown_fields`, so no error is raised.
- `crates/swissarmyhammer-skills/src/deploy.rs:27-44` — `format_skill_md` writes only 8 fields. A dropped key cannot come back.

## Size of the gap

SAH carries: `name` `description` `license` `compatibility` `context` `agent` `metadata` `allowed-tools` `profiles`.

Claude Code also supports these 12, all dropped today: `when_to_use` `argument-hint` `arguments` `disable-model-invocation` `user-invocable` `disallowed-tools` `model` `effort` `background` `hooks` `paths` `shell`.

Only `finish` uses `hooks` now, so only the ralph hook is broken. The other 11 fail as soon as anyone writes them.

Agents are not affected. They deploy verbatim as symlinks into `~/.agents/<name>/`, so keys such as `skills:` and `disallowed-tools:` survive. That asymmetry is the defect.

## Required change

Make the round-trip lossless. Do not add a `hooks` field alone — that fixes one symptom and leaves 11.

1. In `skill_loader.rs`, capture unknown keys with `#[serde(flatten)] extra: BTreeMap<String, serde_yaml_ng::Value>`.
2. Carry `extra` on `Skill` (`crates/swissarmyhammer-skills/src/skill.rs`).
3. In `deploy.rs`, flatten `extra` back out on serialize.
4. Subtract the SAH-internal keys. `profiles` must stay out of the deployed file. Use one shared named constant that both the parse and serialize sides read, so the internal set has a single source of truth.

## Acceptance

- Round-trip test: parse a SKILL.md that carries `hooks`, `model`, `paths`, and `disable-model-invocation`; format it; assert every key survives. This test must fail before the change.
- Test that `profiles` is present after parse and absent after format.
- After `sah init`, `~/.skills/finish/SKILL.md` contains the `hooks` block.

Note: the hook command itself is also broken, in a separate card — `ralph` is not registered in the CLI tool registry, so `sah tool ralph ralph check --` fails. Both cards are needed before the Stop hook works. #bug #init #ralph