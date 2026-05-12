---
assignees:
- wballard
position_column: todo
position_ordinal: f680
title: One agent session per rule (eliminate cross-rule prompt bleed)
---
Today, AVP runs all rules within a RuleSet in a **single, shared agent session**: one `new_session` is created per RuleSet, an init prompt is sent, then each rule is dispatched as a follow-up `prompt()` on the same `session_id`. The model sees prior rules' framing as conversation history, which causes prompt bleed.

**Observed in the field (2026-04-25 qwen test run):**
`security-rules:no-secrets` returned a passed message that explicitly described **input-validation** concerns ("No SQL injection, XSS, command injection, path traversal, XXE, or deserialization issues") — verbatim borrowed from the previous rule in the same session. The rule's own criteria (API keys, tokens, passwords, private keys) weren't applied; the model recycled the prior rule's vocabulary.

**Where it happens:**
- `avp-common/src/validator/runner.rs::execute_ruleset` — doc comment says "evaluates each rule sequentially as part of the conversation, maintaining context across rules." That "maintaining context across rules" is the bug, not a feature.
- The loop at the end of `execute_ruleset` reuses `session_id` for every `PromptRequest`, so each rule's prompt is appended to the running chat history.
- `avp-common/src/context.rs::execute_rulesets` (≈ line 651) reinforces this in its doc: "Each RuleSet runs in a single agent session with rules evaluated sequentially."

**Change required:**
- Each rule must run in its own agent session (`new_session` per rule, prompt with the rule body, collect response, drop the session).
- Drop the "RuleSet session init" mechanism (`render_session_init`, `RuleSetSessionContext`) for the per-rule path — instead, each rule's prompt must be self-contained: include the hook context, changed files / diffs, and the rule criteria. There should be no implicit shared header that lives in the session history.
- Concurrency: rules within a RuleSet can now run in parallel (one session each), bounded by the existing `self.concurrency` semaphore and any per-agent rate-limit budget. Don't pessimize total wall time — that was the implicit benefit of session reuse.
- Audit `RulePromptContext::render` and the rule-prompt partials to confirm a single-shot rule prompt has everything it needs (file content, diffs, hook event, response-format instructions) without relying on prior turns.

**Why:** Validators are independent judgments. They must not influence each other. A prompt-bleed false-pass is worse than a slow validator — it silently reports `passed` on real violations, which is the exact failure mode the user just hit. Isolation is correctness here.

**How to apply:**
1. Refactor `execute_ruleset` so the rule loop creates a fresh session per iteration (or fan out via `join_all` for parallelism).
2. Move all "context the rule needs" into the per-rule prompt body. Verify by reading the rendered prompt in a test — it should make sense in isolation.
3. Update doc comments in `runner.rs` and `context.rs` that currently advertise shared-session semantics.
4. Add a regression test: two rules in the same RuleSet whose prompts would interfere if shared (e.g. one says "always answer in French" and the other expects English JSON) — confirm they're isolated.
5. Re-run the qwen Stop-hook test from the prior task and confirm `no-secrets` no longer borrows `input-validation` language.

**Trade-off accepted:** higher token cost (each rule re-sends its own context) in exchange for correctness and parallelism. Caching at the agent layer (prompt cache, KV reuse) can reclaim some of that later if it matters.

## Review Findings (2026-04-25 13:23)

### Nits
- [x] `avp-common/src/validator/types.rs` (`ExecutedRuleSet` doc comment near line 897) — Still describes the old behavior: "Result of executing an entire RuleSet in a single agent session. A RuleSet execution involves one agent session where all rules are evaluated sequentially as a conversational flow." This contradicts the new per-rule-session model and was specifically called out in the task ("Update doc comments in `runner.rs` and `context.rs` that currently advertise shared-session semantics."). The `runner.rs` and `context.rs` updates landed correctly, but this one was missed. Suggested rewrite: "Result of executing every rule in a RuleSet. Each rule is evaluated in its own fresh agent session, and the per-rule `RuleResult`s are collected here in input order."