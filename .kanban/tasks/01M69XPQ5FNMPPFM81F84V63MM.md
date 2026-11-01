---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6a772npy5k1dqwfdezhbj39
  text: |-
    ### implement — changed
    - double-check agent: replaced `disallowed-tools: "write edit"` with the allow list `tools: "read grep glob mcp__sah__code_context mcp__sah__git"`. The agent has no Edit, Write or Bash tool, and nothing is denied.
    - code-context skill: `grep files` replaced by the host Grep tool.
    - Docs: deleted doc/src/concepts/closed-write-surface.md and its SUMMARY line (the files tool was its instrumented path); fixed tools.md, standards/mcp.md, serve/description.md.
    - Rust: claude-agent classification and title code no longer handle the `files` tool (kept treesitter ops) and tests updated; mcp-proxy filter tests use `git` as the sample tool name; skill_loader fixture uses kanban.
    - Left alone on purpose: hashline and edit-match crates (card ^6n3a6ky), diagnostics crate comments (card ^nfe5pm5), review fleet `read_file`/`glob`/`grep` (ACP client tools), unused partial validator-tools.md (names ACP tools), unified_server regression test that asserts files is not served.
    - New card for ideas/ docs.
    - Tests: cargo nextest -E rdeps(skills)|rdeps(agents)|rdeps(validators)|package(claude-agent)|package(mcp-proxy): 8245 passed, 0 failed. clippy on touched crates: no warnings.
  timestamp: 2026-11-01T00:12:42.550811+00:00
depends_on:
- 01M69XPBKCWV1Q2JAP5MBCW1EK
position_column: doing
position_ordinal: '8480'
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