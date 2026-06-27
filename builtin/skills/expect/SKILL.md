---
name: expect
description: Proactively capture acceptance-criteria-shaped intent from the conversation as a behavioral expectation. Use when the user states how the system should behave ("the coupon should only apply once", "X must do Y", "it should reject an expired token") and offer to save it. Drafts a *.expect.md spec via the `expect expectation create --from-chat` op, pushing for negative/edge criteria ("and it does NOT do X") and domain invariants over frozen literals, then leaves it unapproved for a human to edit-for-intent and approve.
license: MIT OR Apache-2.0
compatibility: Requires the `expect` MCP tool (the `create expectation` op, rendered on the CLI as `expect expectation create`) to draft and doctor the spec; provided by the swissarmyhammer `sah` MCP server. Without it, the skill can recognize intent but cannot persist an expectation.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Expect

Watch the conversation for **acceptance-criteria-shaped intent** and offer to
capture it as a durable, human-owned behavioral expectation — a `*.expect.md`
spec the `expect` tool can run forever after. Recognizing intent the moment it is
expressed is *your* job; the tool only drafts once the human accepts.

## When to offer capture

The user states, in passing, how the system *should* behave — a rule, a
guarantee, an invariant. These are expectations hiding in chat:

- "the coupon should only apply once"
- "an expired token must be rejected with a 401"
- "every column header's count should equal its cards"
- "X must do Y", "it should never Z", "the total has to come off the subtotal"

When you hear one, **offer** (don't silently act): "That sounds like an
expectation — want me to capture it?" Capture intent at the moment it is
expressed; later it evaporates.

## On accept

Invoke the authoring op with the conversation-mined intent:

```
expect expectation create --from-chat
```

Hand it the mined intent plus a **bounded checklist of ~3-5 acceptance
criteria**. The op drafts the `*.expect.md`, loops it through `doctor` until every
field is green, records a candidate observation, and leaves the result
**unapproved** (ledger state `new`). You never hand-fix a spec into validity — the
op's doctor loop does that.

## Mine criteria that actually pin the behavior

A draft is only as good as its criteria. Push past the happy path:

- **Elicit the negative / edge cases.** Agents are weak at failure scenarios and
  drift toward only the stated happy path. Explicitly ask "and what should it
  do *not* do?" — capture criteria of the form "and it does NOT do X" (the second
  apply does NOT stack the discount; an expired token does NOT return 200). A
  rule with no negative case is half a rule.
- **State the right reason.** A criterion must fail when the behavior is wrong,
  not merely when an unrelated value changes — pin the cause, not a coincidence
  (the 401-vs-200 defense). Put the "why" in the body's prose so the example
  alone can't be satisfied for the wrong reason.
- **Prefer invariants over frozen literals.** Push for criteria stated in the
  system's domain language — *how things should be* — that survive incidental
  change. "Each column header's count equals the number of its cards" is an
  invariant; "card X moved, count 2→3" is a brittle frozen literal. An invariant
  catches a whole class of failures and never drifts on incidental data; reach
  for a literal only when there genuinely is one specific expected value.
- **Keep it bounded.** ~3-5 criteria. Rubric focus dilutes past that.

## Hand off — the human owns the result

The drafted spec is intentionally left **unapproved** (`new`). Do not approve it
yourself. Tell the user it is drafted and waiting for them to:

1. **Edit it for intent** — sharpen the prose and criteria so they say exactly
   what correct means.
2. **`approve`** it (`expect observation approve <scope>`) to baseline the golden.

Capturing the intent is your contribution; the human confirms it is right.
