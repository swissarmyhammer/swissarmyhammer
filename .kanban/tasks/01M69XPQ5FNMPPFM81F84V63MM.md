---
assignees:
- claude-code
depends_on:
- 01M69XPBKCWV1Q2JAP5MBCW1EK
position_column: todo
position_ordinal: ffff8e80
project: remove-files-tool
title: Remove files tool references from skills, agents, validators and docs
---
Edit the source in `builtin/`. Never edit `.skills/`; it is generated.

Hits from a first search (check for more):
- `builtin/skills/review/SKILL.md`, `builtin/skills/explore/SKILL.md`, `builtin/skills/code-context/SKILL.md`: they name `read file`, `grep files`, `glob files` or the `files` tool. Tell agents to use the host Read, Grep and Glob tools. Keep `code_context` ops (`grep code`, `get symbol`) as they are. They are a different tool.
- `builtin/agents/reviewer/AGENT.md` and `builtin/agents/double-check/AGENT.md`: same fix. Check their tool allow lists too.
- `builtin/validators/code-security/rules/command-safety.md`: it names the files tool. Fix the wording.
- Docs: `doc/src/concepts/closed-write-surface.md` explains the closed write surface that this work removes. Delete it and its line in `doc/src/SUMMARY.md`, or rewrite it if another part of sah still uses the idea (check the diagnostics watcher first). Fix `doc/src/**`, `README.md` and `standards/mcp.md` where they list the files tool.
- Search `builtin/`, `doc/`, `standards/`, README and the Rust crates for: `read file`, `write file`, `edit file`, `glob files`, `grep files`, `mcp__sah__files`, `files tool`, `hashline`, and any list of tool names that includes `files`. Fix every hit outside `.kanban/` history.
- Check tests that count builtin skills, agents or tools, and fix them.
- Markdown has no validator. Hand-check each edited file: it must read as one coherent document with no dangling reference.

Depends on the card that stops the native deny, so that agents have the host tools when they read the new text.
Done when: no skill, agent, validator or doc names the removed tool, and `cargo nextest run -E 'rdeps(swissarmyhammer-skills) | rdeps(swissarmyhammer-agents) | rdeps(swissarmyhammer-validators)'` is green. #remove-files