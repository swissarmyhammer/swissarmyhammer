---
assignees:
- claude-code
depends_on:
- 01KT9XAK5FB9Y91XBBZDMFMHW9
- 01KT9XB6EWAJXC178REP64YSAQ
position_column: todo
position_ordinal: '8780'
project: claude-hooks
title: End-to-end hook integration tests + documentation
---
Prove the whole path works against a real settings file, and document it.

## E2E tests (scripted fake model + llama ACP server, no GPU)
Set up a temp project dir with a real `.claude/settings.json` (and a `.claude/settings.local.json`) containing command hooks, point a session's cwd at it, and assert the full lifecycle:
- SessionStart fires on new_session.
- A UserPromptSubmit command hook blocks a forbidden prompt (exit 2) end-to-end.
- A PreToolUse `deny` hook prevents a scripted tool call from executing; the model sees the reason.
- PostToolUse additionalContext reaches the model.
- A Stop hook sets hook_should_continue.
- `disableAllHooks: true` disables everything end-to-end.
- Precedence: a user-level (HOME override) hook AND a project hook both fire.
Use a temp HOME (env override) so user-level `~/.claude/settings.json` is testable hermetically.

## Documentation
Add a doc page (under `doc/` mdBook or crate-level docs) covering:
- Which events the llama agent fires (the subset HookEventKind supports) and which Claude events are accepted-but-skipped (forward-compat).
- Settings file locations + precedence/merge order (user → project → local); plugins and managed-policy are NOT supported.
- Supported handler types: command, prompt, agent (prompt/agent backed by the llama model evaluator).
- Tool-name mapping divergence: write matchers against llama-agent tool names (`shell`, `fs_read`, `fs_write`, …, `mcp__<server>__<tool>`), not Claude's (`Bash`/`Edit`/`Write`). Include the canonical tool-name list.
- Exit-code + JSON-stdout contract (reuse the documented semantics already implemented).
- Known ACP-specific notes (PreToolUse now blocks at the dispatch seam; Notification events).

## Acceptance criteria
- All E2E tests green via the scripted model (no real weights).
- Doc page exists and is linked from the project's docs index/SUMMARY.
- `really-done` style verification: cargo test for the touched crates passes; clippy clean.