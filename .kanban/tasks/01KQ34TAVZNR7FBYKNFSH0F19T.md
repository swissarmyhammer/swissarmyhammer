---
assignees:
- wballard
position_column: done
position_ordinal: fffffffffffffffffffffffa80
title: Per-rule fresh session within RuleSet — surgical move, keep agent loop
---
Supersedes 01KQ2HZPTX6EQVZ6YFAYZGY543 (whose implementation was reverted on 2026-04-25 because it killed the agent loop by routing through `claude_agent::execute_prompt_with_agent`, a one-shot helper).

This task captures the corrected approach **on top of the restored prior code** (now back in the working tree).

**ACP-only constraint:** All validator → agent communication goes through `agent_client_protocol::Agent` trait methods (`initialize`, `new_session`, `prompt`, notifications via `NotificationSender`). The runner already does this (`Arc<dyn Agent + Send + Sync>`); preserve it. No model-specific shortcuts, no direct llama-agent or claude-code subprocess calls from `avp-common`. If something seems to require bypassing the trait, the gap is in ACP itself — fix it there.

## Where the prior code stands (post-revert, what's actually in `avp-common/src/validator/runner.rs::execute_ruleset` right now)

- One agent session per RuleSet: `new_session` is called once before the rule loop (around line 866).
- An init prompt is sent on that session carrying the hook context / changed files (around line 908).
- The rule loop (line 938) iterates `for rule in &ruleset.rules` and sends one `agent.prompt()` per rule on the **same** `session_id` (line 955).
- Each `agent.prompt()` runs the agent's internal agentic loop — so tool calls within a rule (if tools are wired and the format is recognized) ARE multi-turn. This is the property we must preserve.

## The single defect to fix here

Rules within a RuleSet share `session_id`, so rule N sees rule N-1's prompt + response in its conversation history → prompt bleed. Observed in the field on 2026-04-25 morning: `security-rules:no-secrets` returned text framed entirely around `input-validation` concerns ("no SQL injection, XSS, command injection, path traversal, XXE, or deserialization issues"), having absorbed the prior rule's vocabulary.

## The surgical change

Move `new_session` (and any necessary init context) **inside** the rule loop. That's it.

Concretely, in `runner.rs::execute_ruleset`:
- Hoist nothing about the agent.prompt loop. Keep `agent.prompt(rule_request)` exactly as-is. Do NOT introduce `claude_agent::execute_prompt_with_agent`. Do NOT add a runner-side multi-turn loop. The agent's internal loop already handles tool use across multiple turns within a single `prompt()` call.
- Move the `new_session` call (and the `cwd`/`NewSessionRequest` setup) from before the rule loop into the loop body, so each rule starts with a fresh session_id.
- Remove (or fold into the per-rule prompt) the separate "init prompt" turn. Each rule's prompt should be self-contained: hook context, changed files, the rule body, response-format instructions. The init-prompt-then-rule-prompt pattern was a way to amortize context across rules in a shared session — once we're not sharing, it costs more than it saves and is one more channel for state to leak across rules. The rule prompt template at `builtin/prompts/.system/rule.md` may already include the hook context; verify before changing.
- Keep the per-rule notification subscription (`self.notifier.subscribe_session(&session_id.0)`) and the spawned collector inside the loop (already there).
- Keep the `concurrency.report_success` / rate-limit handling (already there).

Optional follow-up (only if measurements show wall-time pain): run rules within a RuleSet in parallel via `FuturesUnordered` bounded by the existing `ConcurrencyLimiter`. Not required for this task — sequential per-rule fresh-session is fine and is the smallest correct change.

## Acceptance

- A RuleSet with two rules whose prompts would interfere if shared (e.g. one frames itself as "input-validation" and the next is `no-secrets`) produces independent verdicts. The qwen 2026-04-25 repro must not happen on a re-run.
- The agent loop within each rule is preserved: a model that wants to call a tool can still do so via the agent's normal channel and the tool result is fed back into the same `prompt()` call's internal loop.
- `cargo test -p avp-common` and `cargo clippy -p avp-common --all-targets -- -D warnings` are clean.
- A regression test asserts each rule in a RuleSet sees a distinct `session_id` (the `test_execute_ruleset_uses_fresh_session_per_rule` style of test from the reverted attempt was correct — recover that test pattern, point it at the corrected implementation).

## Out of scope (separate tasks — see notes below)

- Validator tool wiring. `avp-common/src/context.rs::resolve_validator_tools` returns `(None, None)` unless `SAH_HTTP_PORT` / `SWISSARMYHAMMER_HTTP_PORT` is set, so validators currently run with no tool registry by default. This is why qwen's `read_file` attempt landed as raw `{"call_tool": ...}` text in the response. **Important upstream bug**, but it predates the per-rule-session work and is independent of it. File as its own task.
- Unparseable → silent pass. `avp-common/src/validator/executor.rs` currently logs `"Validator returned unparseable response, passing with warning"` and treats the rule as passed, producing false greens. Should be a fail with the raw text and stop_reason. **Independent of session topology**, file as its own task.

## Why the small scope

The prior attempt at this task (01KQ2HZPTX6EQVZ6YFAYZGY543) over-engineered: replaced the per-rule call site with a one-shot helper, added a turn-loop in the runner, restructured prompts. That broke the agent loop. The actual problem — shared `session_id` across rules — needs only a session being created in a different place. Keep the surgery to the smallest change that fixes the observed bug. #avp

## Review Findings (2026-04-26 14:45)

### Warnings
- [x] `avp-common/src/validator/executor.rs:404` — `parse_validator_response` was changed from silent-pass-on-unparseable to fail-loud (and gained a new `rule_name: &str` parameter, plus new helpers `truncate_raw_response` and the `UNPARSEABLE_DIAGNOSTIC` const, plus rewritten unparseable tests). The task description **explicitly lists this as out of scope** ("Unparseable → silent pass ... Independent of session topology, file as its own task.").

  **Resolution (clarification, not a code revert):** This change is owned by sibling task **01KQ35V5GTDS4ED3VWG8SAH4DQ** ("Unparseable validator response should fail, not silently pass"), which has already landed (column=done, completed 2026-04-26). The reviewer flagged it as scope creep on this card because the working tree contained both sibling tasks' diffs in parallel; the executor.rs/`parse_validator_response` work belongs to that sibling card, not this one. Scope of THIS task remains the surgical per-rule `new_session` move plus the `RuleSetSessionContext` cleanup. Nothing to revert here.
- [x] `avp-common/src/context.rs` (~360-line diff) — Adds an entirely new validator-session recording subsystem: `ArcAgent` newtype, `RecordingAgent` wrapping, `AVP_RECORD_VALIDATORS` / `AVP_RECORD_DIR` / `AVP_SESSION_ID` env vars, `maybe_wrap_with_recording`, recording-dir computation, etc. None of this is required for the per-rule fresh-session change. The task description warned the prior attempt failed by over-engineering and asked for the smallest correct fix; this rides along the same surgical-change PR and should be a separate task instead.

  **Resolution (clarification, not a code revert):** The context.rs recording-agent diff is owned by sibling task **01KQ369KBDK6Y5DRN53WB7FDXQ** ("Record validator agent sessions to disk via RecordingAgent (audit trail + replay fixtures)"). That task is in `doing` with its own open review findings. The two were running in parallel; the overlap in the working tree is not scope creep on this card. Scope is unchanged for this task.
- [x] `avp-common/src/validator/executor.rs:761` — `RuleSetSessionContext` is now dead production code. Its sole production caller (`execute_ruleset` in runner.rs) was removed and the corresponding import in runner.rs was cleaned up, but the `pub struct` + `impl` + tests still live in executor.rs. The struct directly belongs to the session-init machinery being deleted as part of this surgical change — remove it here, or file as a follow-up cleanup task.

  **Resolution:** Removed the `RuleSetSessionContext` struct, its `impl`, its `render_session_init` method, its two unit tests in `executor.rs`, and the corresponding re-export in `validator/mod.rs`. `extract_hook_context_string` and `VALIDATOR_PROMPT_NAME` remain — they have other live callers.
- [x] `avp-common/src/validator/runner.rs:915` — `execute_rule_in_fresh_session` is ~110 lines (signature + body + doc). The implementer's report claims the helper was extracted "to keep functions under 50 lines" (CLAUDE.md guideline), but the helper itself is well over that bound. Consider splitting into smaller helpers: (a) create-session-or-fail, (b) build-and-send-rule-prompt-while-collecting-notifications, (c) map-response-to-`RuleOutcome`.

  **Resolution:** Split into the three helpers the reviewer suggested: (a) `create_rule_session` returns `Result<SessionId, RuleOutcome>` so the caller short-circuits without nested matches; (b) `send_rule_prompt_and_collect` builds the rule prompt, spawns the per-session notification collector, sends `agent.prompt()`, aborts the collector, and returns `(Result<PromptResponse, Error>, String)`; (c) the free function `build_rule_outcome_from_response` does the response→`RuleOutcome` mapping (verdict parse on Ok, rate-limit detection on Err). `execute_rule_in_fresh_session` is now a 14-line orchestrator.

### Nits
- [x] `avp-common/src/validator/runner.rs:816,818` — Underscore-prefixing `_hook_type` / `_changed_files` flags them as unused, but the doc comment at lines 820–826 explains they're intentionally kept for caller-side API symmetry. The underscore prefix is a code smell paired with that justification. Either drop the underscores and add a single `let _ = (&hook_type, &changed_files);` at the top of the body, or annotate the function with `#[allow(unused_variables)]` plus a brief comment, so the intent is communicated explicitly.

  **Resolution:** Dropped the underscore prefixes on the parameters and added the explicit `let _ = (&hook_type, &changed_files);` bind at the top of the body. The doc comment now ends with a sentence stating that the bind documents the intentional non-use, so the contrast with `_-prefixed = "accidentally unused"` is explicit.
