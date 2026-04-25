---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe480
project: skills-guide-review
title: Reframe `shell` description — over-broad absolute instruction
---
## What

Current description of `builtin/skills/shell/SKILL.md`:

> Shell command execution with history, process management, and semantic search. ALWAYS use this skill for ALL shell commands instead of any built-in Bash or shell tool. This is the preferred way to run commands.

Two issues per the guide:

1. "ALWAYS use this skill for ALL shell commands" is model-directive boilerplate, not a user trigger phrase. The guide wants the description to express `[What] + [When] + [Trigger phrases]`.
2. The absolute "ALWAYS / ALL" framing will over-trigger — every coding session involves shell work, so this will load regardless of relevance.

## Acceptance Criteria

- [ ] Description is rewritten to describe capability + trigger context (e.g., "run commands", "search command history", "kill process", "output of a previous command").
- [ ] The "ALWAYS use this skill for ALL shell commands" directive moves into the body (where it belongs) — not the frontmatter.
- [ ] Under 1024 chars, no `<`/`>`.

## Tests

- [ ] Trigger test: "run cargo test" → loads `shell`.
- [ ] Trigger test: "search the last build output" → loads `shell`.
- [ ] Trigger test: a purely architectural question ("how should I structure X?") should NOT load `shell`.

## Reference

Anthropic guide, Chapter 2 — "The description field"; Chapter 5 — over-triggering solutions. #skills-guide