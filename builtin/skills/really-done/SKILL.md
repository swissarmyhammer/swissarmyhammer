---
name: really-done
description: Verify the work before you claim it is done. Use this skill when the user says "really done", "are we done", "ready to ship", "ready to commit", "is this passing", or when you are about to claim that work is complete, fixed, or passing. Also use it before you commit or create a pull request. You must run verification commands and check the output before any success claim. Evidence must always come before a claim.
license: MIT OR Apache-2.0
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Really Done

**Evidence must always come before a claim.** Claiming that work is complete, without verification, is dishonesty, not efficiency.

## The Iron Law

```
NO COMPLETION CLAIMS WITHOUT FRESH VERIFICATION EVIDENCE
```

If you have not run the verification command in *this* message, you cannot claim that it passes.

## The Gate

Before any status claim or expression of satisfaction:

1. **Identify** the command that proves the claim
2. **Run** it fresh, in full
3. **Read** the full output. Check the exit code. Count the failures.
4. **Verify** that the output matches the claim.
5. **Then** state the claim, with the evidence.

Skipping any step means you are lying, not verifying.

## Adversarial Sign-Off (advisory gate)

Running the verification command above is the **main, required step**. The Iron Law is not negotiable, and this step does not replace it. After the command passes, if there are **code changes to verify**, get a second review before you claim the work is done:

1. **Skip this gate if there is no diff.** If no code changed, there is nothing to review. Skip this gate entirely.
2. **Start the critic.** Launch the `double-check` agent with the **Task tool** (`subagent_type: double-check`) for a critical review of the changes.
3. **Read the verdict.** It returns `PASS` or `REVISE` with findings.
   - `PASS` — go on to the completion claim.
   - `REVISE` — either fix the findings, **or** continue past them with a short, recorded reason, for example a kanban task comment, that explains why they are acceptable. You must not ignore them silently.
4. **Limit the loop.** Act on the findings, and start `double-check` again **at most once**. Do not review the same tree over and over. After one re-check, either claim the work is done, or continue with a recorded reason.

This gate is **advisory**. It surfaces risk and informs the decision, but the caller may still proceed. The verification command run before any claim remains a requirement. You cannot waive it.

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
| Behavior change complete | Full **existing** test module for the touched area runs green, and the inverse or edge path is exercised | Only the tests you wrote pass |

## Red Flags — STOP

- Hedging: "should", "probably", "seems to"
- Premature satisfaction: "Great!", "Perfect!", "Done!"
- You are about to commit, push, or open a PR without verification
- You trust an agent's success report
- Partial verification
- "Just this once" or "I am tired"
- **Any wording implying success without verification**

## Rationalizations vs Reality

| Excuse | Reality |
|--------|---------|
| "Should work now" | Run the verification. |
| "I'm confident" | Confidence is not evidence |
| "Just this once" | No exceptions |
| "Linter passed" | A linter is not a compiler |
| "Agent said success" | Verify independently |
| "I'm tired" | Exhaustion is not an excuse |
| "Partial is enough" | Partial proves nothing |

## Patterns

**Tests:** Run the tests, see `34/34 pass`, then claim success. Do not claim success because it "looks correct".

**Regression (TDD red-green):** Write the test, run it and see it pass, revert the fix, run it again and it **must fail**, restore the fix, then run it again and see it pass.

**Build:** Run the build, see exit code 0, then claim success. A passing linter is not the same as a passing build.

**Requirements:** Read the plan again, build a checklist, verify each item, then report gaps or completion.

**Agent delegation:** When an agent reports success, check the VCS diff, verify the changes, then report the real state. Do not trust the report alone.

**Existing suite, not just new tests:** For a behavior change, run the **full** test module that already existed for the area you touched, for example `pytest path/to/test_module.py`. Do not run only the tests you wrote. Tests you wrote to match your own change pass by design; they are not evidence that the change is complete. Before you claim the work is done, ask: *what input would a skeptic try that my tests do not cover?* Check the inverse direction especially, for example reading back what you can now write, and the sibling paths, for example every other place that uses the token or flag you changed.

## When to Apply

Before any:
- A success or completion claim, whether an exact phrase, a paraphrase, or an implication
- Expression of satisfaction
- Commit, PR, task completion
- Moving to the next task
- Delegating to agents
- Claiming that a code change is done: get sign-off from the `double-check` agent first (advisory; see above)

## Why It Matters

Past failures include: the user's trust broken, when the user said "I don't believe you"; undefined functions shipped; incomplete features delivered; and time wasted on false completion, followed by redirection and rework. Honesty is a core value.

## Bottom Line

**No shortcuts.** Run the command. Read the output. Then claim the result. This rule is not negotiable.
