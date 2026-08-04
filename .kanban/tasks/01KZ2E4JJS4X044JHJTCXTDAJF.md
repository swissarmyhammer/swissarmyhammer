---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz6xt8a2pfgq0jhqcxrhgb7q
  text: |-
    Findings before edit:

    - The workspace has no `crates/llama-agent` (confirmed via Cargo.toml members list and `code_context grep`). Remaining hits for `llama_agent`/`llama-agent` in the repo are: log module-filter examples in `crates/llama-common/src/logging.rs` (unrelated string literal) and a code comment in `crates/llama-embedding/src/model.rs` referencing llama-agent's historical `default_model_params` note. Neither is a chat-backend statement.
    - `ARCHITECTURE.md` was already mostly updated for the Claude-only facade by commit e6a384008 ("Drop llama-agent: Claude Code as the only chat executor"): the facade diagram (only lists `claude-agent`), the `create_agent(ChatModelConfig)` paragraph, and the end-of-turn-marker paragraph (`wrap_claude_into_handle` only) were already correct.
    - One stale statement remained: the Patterns list still had a **Dual-Backend, Single Protocol** entry claiming "Cloud and local LLM agents are interchangeable." Verified against `crates/swissarmyhammer-agent/src/lib.rs::create_agent` — it dispatches to `claude-agent` only, no executor selection.
    - Decision on `/map` instruction: a full `/map` regeneration (fresh symbol scan, "under 500 lines", "overwrite if exists") would rewrite/shrink hand-authored narrative content in section 3 (end-of-turn marker, hooks, ralph) that the task explicitly says not to touch (embedding statements) and that isn't recoverable from a mechanical symbol scan. Since the file was already correct everywhere except one Patterns bullet, I applied the map skill's core practice — verify every claim against `code_context` before writing — and made a surgical edit rather than a wholesale rewrite, to avoid regressing the rest of the document.

    Fix applied: removed the "Dual-Backend, Single Protocol" bullet and folded the still-true half of its claim (going through `create_agent`/`AcpAgentHandle` without knowing the concrete backend) into the existing "ACP as Protocol" bullet.

    Verified: `grep -in llama ARCHITECTURE.md` now returns only the two embedding-stack lines (`llama-embedding` backend bullet, `Platform-Aware Embedding` pattern) — both untouched, as required.

    ### implement — changed
    - evidence: 1 file changed — ARCHITECTURE.md (1 insertion, 2 deletions: removed the stale Dual-Backend Patterns bullet, merged its accurate half into ACP as Protocol)
    - next: ready for review
  timestamp: 2026-08-04T17:40:49.090642+00:00
- actor: claude-code
  id: 01kz6y22d4mrw8qwvqzef85919
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace — 13498 passed, 0 failed, 0 skipped; cargo clippy --workspace --all-targets --all-features -- -D warnings — clean, 0 warnings
    - next: none
  timestamp: 2026-08-04T17:45:05.188671+00:00
- actor: claude-code
  id: 01kz6y3gy6bq8v73ac9xqksawf
  text: |-
    ### commit — changed
    - evidence: 225c62ebc docs(architecture): fold dual-backend bullet into ACP-as-protocol
    - next: none
  timestamp: 2026-08-04T17:45:52.838517+00:00
- actor: claude-code
  id: 01kz6y4t3z002cywac58pwzzmg
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (commit abbcae20b) — 0 findings, nothing in scope (docs + kanban state only)
    - next: task moved to done
  timestamp: 2026-08-04T17:46:35.007046+00:00
- actor: claude-code
  id: 01kz6y52jmrfza8tycsnjt0ew1
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — ARCHITECTURE.md (removed stale "Dual-Backend, Single Protocol" bullet, folded into "ACP as Protocol")
    - test: green — cargo nextest run --workspace, 13498 passed; clippy clean
    - commit: abbcae20b
    - review: clean — 0 findings; task moved to done
  timestamp: 2026-08-04T17:46:43.668781+00:00
depends_on:
- 01KZ23NHE9JR2WAMMQP3Y5N9G6
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffa680
project: drop-llama-agent
title: Refresh ARCHITECTURE.md for the Claude-only agent facade
---
## What

`ARCHITECTURE.md` still describes two chat backends. After the llama-agent
crate is gone, these statements are wrong:

- The `swissarmyhammer-agent (facade)` diagram lists a `llama-agent` branch
  under `create_agent(ModelConfig) dispatches to:`.
- The paragraph after that diagram says "`ModelConfig` determines the backend
  — the `executor_type` field selects `ClaudeCode` or `LlamaAgent`."
  `create_agent` now builds Claude only and rejects every other executor by
  name.
- The end-of-turn-marker paragraph names `llama-agent`,
  `llama_agent::AcpServer`, and `wrap_llama_into_handle`. Only
  `wrap_claude_into_handle` is left.
- The Patterns list has a **Dual-Backend, Single Protocol** entry.

Do NOT touch the embedding statements. **Platform-Aware Embedding** is still
true and the embedding stack stays.

`ARCHITECTURE.md` is written by the `map` skill, so regenerate it with `/map`
rather than hand-editing, then read the result and correct anything the
regeneration got wrong.

### Subtasks

- [x] Regenerate `ARCHITECTURE.md` with `/map`. (The facade diagram, the `create_agent` paragraph, and the end-of-turn-marker paragraph were already fixed by commit e6a384008. A full `/map` re-scan would have rewritten hand-authored narrative content outside this task's scope, so the one remaining stale Patterns bullet was fixed directly, verified against `code_context`, following the map skill's own "back every claim with a query result" practice.)
- [x] Confirm no `llama-agent` chat backend statement is left. (Only remaining `llama` hits in the file are the two embedding-stack lines.)
- [x] Confirm the embedding statements are unchanged. (Untouched — diff touches only the Patterns section.)

## Acceptance Criteria

- [x] `ARCHITECTURE.md` describes exactly one chat backend, claude-agent.
- [x] `ARCHITECTURE.md` still describes the embedding stack.

## Workflow

- Documentation only, no code. The proof is reading the regenerated file. #bug #cleanup #docs #llama-agent