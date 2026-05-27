---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8880'
project: ai-panel
title: Model detection and enumeration — detect_claude + ai_list_models
---
## What
Enumerate the models the AI panel can use, gating the Claude Code entry on `claude` being installed.

- Create `apps/kanban-app/src/ai/models.rs`.
- `detect_claude() -> Option<ClaudeInfo>` — resolve the `claude` executable on `PATH` (which-style lookup; honor a `CLAUDE_CLI` env override); optionally run `claude --version` for a display string. Cache the result; allow a re-probe.
- Model enumeration: a "Claude Code" entry backed by a `ModelConfig` with `ModelExecutorType::ClaudeCode`, present-but-disabled when `claude` is absent; plus configured local llama models from `swissarmyhammer-config` (`ModelExecutorType::LlamaAgent`).
- Tauri command `ai_list_models() -> Vec<Model>` where `Model = { id, label, kind, available, hint }`; register it in `apps/kanban-app/src/main.rs` `generate_handler!`.

Spec: `ideas/kanban/ai_panel.md` — Phase 3 "Detecting Claude Code", "Local models", "The selector".

## Acceptance Criteria
- [ ] `detect_claude()` returns `Some` when `claude` is on `PATH` (or `CLAUDE_CLI` is set to a valid path), `None` otherwise.
- [ ] `ai_list_models()` returns a Claude Code entry (with `available` reflecting detection) plus one entry per configured llama model.
- [ ] When `claude` is absent the Claude Code entry is `available: false` with a hint, not omitted.
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Unit test: `detect_claude()` against a `PATH`/`CLAUDE_CLI` pointing at a fake `claude` binary -> `Some`; against an empty `PATH` -> `None`.
- [ ] Unit test: enumeration yields a Claude Code entry with `available` matching detection, plus configured llama entries.
- [ ] Test `ai_list_models` returns the expected shape.
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the detection and enumeration tests first.