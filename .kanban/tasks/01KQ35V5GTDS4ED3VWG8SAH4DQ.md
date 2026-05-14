---
assignees:
- wballard
position_column: done
position_ordinal: fffffffffffffffffffffff980
title: Unparseable validator response should fail, not silently pass
---
Today, when a validator's final response can't be parsed as the required pass/fail JSON, `avp-common/src/validator/executor.rs` logs:

```
Validator returned unparseable response, passing with warning: "<the raw text>"
```

…and treats the rule as **passed**. That produces silent false greens. We caught it on 2026-04-25 only because the log was open in a terminal — the validator reported "passed" upstream while the model was, in the raw text, attempting (and failing) to call `read_file`.

**Where it lives:** `avp-common/src/validator/executor.rs` — search for the `passing with warning` literal; the call site is the parse-fallback branch of `parse_validator_response` (or its caller).

**Change:** Flip the policy.
- Unparseable response → return `ValidatorResult::fail(...)`, not pass.
- Failure message must include: the raw text (truncated to a sane upper bound, e.g. 4 KB, with a "[truncated]" marker if it's longer), the agent's `stop_reason`, and a one-sentence diagnostic ("Validator response was not valid JSON of the required schema; this typically means the model emitted a tool-call attempt the agent could not parse, or returned narrative without the required JSON object.")
- Severity follows the rule's `effective_severity(ruleset)` like any other failure — no special-casing.
- Keep the `tracing::warn!` log line for visibility, but change the wording so it no longer claims "passing" — e.g. `Validator returned unparseable response, failing rule '...'`.

**Why:** A validator that "passes" because the model's output couldn't be understood is an antipattern — it teaches users that green = safe, when in this branch green just means "we couldn't tell." Failing loudly forces the upstream tasks (`01KQ35MHFJQPMEKQ08PZKBKFY0` tools wiring + `01KQ35KFJXJ70GNB4ZPRJD6R43` lenient parser) to actually be solved instead of papered over.

**Tests required:**
- Unit test feeding a non-JSON narrative response into the parse path: rule must fail, with the raw text in the failure message and the stop_reason captured.
- Unit test feeding a syntactically valid JSON object that doesn't match the pass/fail schema (e.g. `{"foo": "bar"}`): same fail behavior.
- Existing pass-path test (valid `{"status":"passed", ...}` and `{"status":"failed", ...}`) must continue to pass.
- Negative integration: a Stop-hook run where one rule's response is unparseable produces a non-zero validator exit / blocking output appropriate for the rule's severity (this verifies the policy change actually flows through to hook decisions, not just the parse helper).

**Pairs with:** `01KQ35KFJXJ70GNB4ZPRJD6R43` (lenient parser) and `01KQ35MHFJQPMEKQ08PZKBKFY0` (tool wiring). Once those two land, this branch should rarely fire — but when it does, it must fail. Order doesn't matter; this can land independently.

**Risk:** rules currently passing on this branch will start failing. That is the intended behavior — those passes were not real. Watch for any rule whose prompt is so verbose / token-heavy that the model frequently times out or returns non-JSON; if there are persistent offenders, fix the prompt, not the policy. #avp