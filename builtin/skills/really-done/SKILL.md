---
name: really-done
description: Verify work before claiming it done. Use when the user says "really done", "are we done", "ready to ship", "ready to commit", "is this passing", or when about to claim work is complete, fixed, or passing. Also use before committing or creating PRs. Requires running verification commands and confirming output before any success claim — evidence before assertions, always.
license: MIT OR Apache-2.0
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Really Done

**Evidence before claims, always.** Claiming work complete without verification is dishonesty, not efficiency.

## The Iron Law

```
NO COMPLETION CLAIMS WITHOUT FRESH VERIFICATION EVIDENCE
```

If you haven't run the verification command in *this* message, you cannot claim it passes.

## The Gate

Before any status claim or expression of satisfaction:

1. **Identify** the command that proves the claim
2. **Run** it fresh, in full
3. **Read** full output, check exit code, count failures
4. **Verify** output matches the claim
5. **Then** state the claim — with evidence

Skip any step = lying, not verifying.

## What Counts as Proof

| Claim | Requires | Not sufficient |
|-------|----------|----------------|
| Tests pass | Test command: 0 failures | "Should pass" |
| Linter clean | Linter: 0 errors | Partial check |
| Build succeeds | Build: exit 0 | Linter passed |
| Bug fixed | Original symptom test passes | Code changed |
| Regression test works | Red-green-red cycle verified | Test passes once |
| Agent completed | VCS diff shows changes | Agent says "success" |
| Requirements met | Line-by-line checklist | Tests pass |

## Red Flags — STOP

- Hedging: "should", "probably", "seems to"
- Premature satisfaction: "Great!", "Perfect!", "Done!"
- About to commit/push/PR without verification
- Trusting agent success reports
- Partial verification
- "Just this once" / "I'm tired"
- **Any wording implying success without verification**

## Rationalizations vs Reality

| Excuse | Reality |
|--------|---------|
| "Should work now" | RUN the verification |
| "I'm confident" | Confidence ≠ evidence |
| "Just this once" | No exceptions |
| "Linter passed" | Linter ≠ compiler |
| "Agent said success" | Verify independently |
| "I'm tired" | Exhaustion ≠ excuse |
| "Partial is enough" | Partial proves nothing |

## Patterns

**Tests:** Run → see `34/34 pass` → claim. Not "looks correct".

**Regression (TDD red-green):** Write → run (pass) → revert fix → run (MUST FAIL) → restore → run (pass).

**Build:** Run → exit 0 → claim. Linter passing ≠ build passing.

**Requirements:** Re-read plan → checklist → verify each → report gaps or completion.

**Agent delegation:** Agent reports success → check VCS diff → verify changes → report actual state. Don't trust the report alone.

## When to Apply

Before any:
- Success/completion claim (exact phrase, paraphrase, or implication)
- Expression of satisfaction
- Commit, PR, task completion
- Moving to the next task
- Delegating to agents

## Why It Matters

Past failures: trust broken when the user said "I don't believe you"; undefined functions shipped; incomplete features delivered; time wasted on false completion → redirect → rework. Honesty is a core value.

## Bottom Line

**No shortcuts.** Run the command. Read the output. Then claim the result. Non-negotiable.
