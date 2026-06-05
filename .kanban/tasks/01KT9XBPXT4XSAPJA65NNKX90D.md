---
assignees:
- claude-code
depends_on:
- 01KT9XAK5FB9Y91XBBZDMFMHW9
- 01KT9XB6EWAJXC178REP64YSAQ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffef80
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

## Review Findings (2026-06-04 16:22)

### Warnings
- [x] `doc/src/reference/llama-claude-hooks.md` (Events fired vs. accepted-but-skipped → "Accepted but skipped" section) — The doc claims all 15 listed kinds are "silently dropped when registrations are built." That is only true for the 6 kinds whose `HookEventKindConfig → HookEventKind` conversion returns `Err(UnsupportedEventKind)` and are `continue`d in `build_registrations_with_context` (`PermissionRequest`, `SubagentStart`, `SubagentStop`, `PreCompact`, `Setup`, `SessionEnd` — see `crates/agent-client-protocol-extras/src/hook_config.rs` `TryFrom<HookEventKindConfig>` and `build_registrations`). The other 9 (`PostCompact`, `TeammateIdle`, `TaskCompleted`, `Elicitation`, `ElicitationResult`, `InstructionsLoaded`, `ConfigChange`, `WorktreeCreate`, `WorktreeRemove`) convert to a valid `HookEventKind` via `Ok(...)`, so registrations ARE built for them — they simply never fire because no production seam ever constructs those `HookEvent` variants (they appear only in tests). Net behavior (never fires) is correct, but the stated mechanism is inaccurate for those 9. Acceptance criterion 2 requires the page be accurate against the implementation. Suggested fix: split the list into "dropped at registration build (no ACP HookEventKind)" — the 6 — vs. "registered but never fired (no seam emits the event)" — the 9 — or reword the single sentence so it does not assert all are dropped at build time.
  RESOLVED: Section retitled "Accepted but never fired (forward-compatible)" and split into two subsections — "Dropped at registration build (no ACP `HookEventKind`)" listing the 6 `Err(UnsupportedEventKind)` kinds, and "Registered but never emitted (no seam constructs the event)" listing the 9 `Ok(...)` kinds — with a closing note that net behavior is identical but only the first group is dropped at build. Intro/heading reworded to match.

### Nits
- [x] `doc/src/reference/llama-claude-hooks.md` (tool-name list, `shell` entry) — Says `shell` "also matches `terminal`". Per `crates/llama-agent/src/agent.rs` both `terminal` and `shell` map to `CapabilityType::Terminal` for permission purposes, but a `PreToolUse` matcher is regex-tested against the bare emitted tool name; a literal `shell` matcher does not also match a tool literally named `terminal`. The parenthetical reads as if the matcher value `shell` also matches `terminal`, which it does not. Consider rewording to "the model may emit either `shell` or `terminal`; match whichever name your model emits" to avoid implying matcher-level equivalence (same applies to the `fs_read`/`fs_write` alias parentheticals).
  RESOLVED: Reworded the canonical tool-name list so each entry says the model "may emit" the listed aliases (which all resolve to the same capability) but a matcher targets whichever name the model actually emits — explicitly noting `shell` does not match a tool emitted as `terminal`, and likewise for the fs aliases. Added clarifying lead-in and an alternation example (`shell|terminal`, `fs_read|read_file`) for matching multiple aliases.