---
name: test-driven-development
description: Follow test driven development. Use this any time you are coding, all the time.
---

# Test-Driven Development

## The Iron Law

**No code without a failing test first.**

Violating the letter of this rule is violating the spirit. There are no "spirit of TDD" exceptions. The discipline IS the practice.

## The Cycle

Every change follows RED → GREEN → REFACTOR. One behavior at a time. No skipping steps.

### RED: Write One Failing Test

Write a test for exactly one behavior. Run it. Watch it fail.

**The failure must be correct** — the test fails because the behavior doesn't exist yet, not because of a typo, import error, or wrong assertion. If the test passes immediately, you are testing existing behavior. Fix the test.

You MUST run the test and see it fail before proceeding. This is not optional. This is not skippable. Seeing the failure proves your test actually tests something.

### GREEN: Make It Pass

Write the **minimum code** to make the failing test pass. No more. Do not implement the next feature. Do not refactor. Do not add "obvious" improvements.

Run all tests. Every test must pass. If any test fails, fix the implementation — never fix a test to match wrong behavior.

### REFACTOR: Clean Up

All tests are green. Now improve the code: extract duplication, improve names, simplify structure. Run tests after each change. If any test breaks, undo the last change.

**Never refactor while RED.** Get to GREEN first.

### REPEAT

Pick the next behavior. Write a failing test. One behavior at a time.

## Vertical Slices, Not Horizontal

**WRONG**: Write all tests first, then all implementation.

**RIGHT**: One test → one implementation → repeat.

Writing tests in bulk tests *imagined* behavior. You outrun your headlights, committing to test structure before understanding the implementation. Each RED-GREEN cycle should respond to what you learned from the previous one.

```
WRONG (horizontal):     RIGHT (vertical):
  RED:   t1 t2 t3 t4      RED→GREEN: t1→i1
  GREEN: i1 i2 i3 i4      RED→GREEN: t2→i2
                           RED→GREEN: t3→i3
```

Start with a **tracer bullet**: one test that proves the end-to-end path works. Then build outward from that foundation.

## What to Test

Test **behavior through public interfaces**, not implementation details.

A good test describes **what** the system does, not **how** it does it. If you rename an internal function and tests break but behavior hasn't changed, those tests were wrong.

**Mock only at system boundaries**: external APIs, databases, time, randomness. Never mock your own code or internal collaborators. If something is hard to test without mocking internals, the design needs to change — not the test.

### Coverage Requirements

Every new function or method gets a test. Target 80%+ coverage overall.

For each behavior, test the happy path AND: what happens with empty input, null/missing values, invalid types, boundary values, error conditions, and concurrent access where relevant. See [edge-cases.md](edge-cases.md) for the complete checklist.

**Language-specific patterns**: See [rust-testing.md](rust-testing.md) for Rust (`cargo nextest`, `proptest`, trait-based mocking, `insta` snapshots). See [typescript-testing.md](typescript-testing.md) for TypeScript/JavaScript (Vitest, Testing Library, `fast-check`, component/hook testing).

## Verification Checklist

After each RED-GREEN-REFACTOR cycle, confirm:

- [ ] Test failed before implementation (you watched it fail)
- [ ] Test failed for the right reason (missing behavior, not typo)
- [ ] Implementation is minimal (only what this test requires)
- [ ] All tests pass (not just the new one)
- [ ] Test output is clean (no warnings, no skipped tests)
- [ ] Test uses real code paths (mocks only at system boundaries)

## When You Already Wrote Code

You wrote production code before writing a test. This happens. Here is what to do:

**Delete it. Start over with a failing test.**

- Do not keep it as "reference"
- Do not "adapt" it while writing tests
- Do not look at it while writing the test
- Delete means delete

This is not wasteful. The code you wrote is unverified. Tests written after implementation prove nothing — they test what the code does, not what it should do. Tests written first define correct behavior, then the implementation proves it satisfies that definition.

## Rationalizations (All Are Wrong)

| You think | Reality |
|-----------|---------|
| "Too simple to need a test" | Simple code breaks. The test takes 30 seconds. Write it. |
| "I'll write tests after" | Tests that pass immediately prove nothing about correctness. |
| "I already manually tested it" | Manual testing is not systematic, not repeatable, not recorded. |
| "Deleting my work is wasteful" | Keeping unverified code is technical debt. Sunk cost fallacy. |
| "Tests after achieve the same thing" | Tests-after ask "what does this do?" Tests-first ask "what should this do?" |
| "I'm following the spirit of TDD" | Violating the letter IS violating the spirit. No exceptions. |
| "This is different because..." | It isn't. Write the failing test first. |
| "Keep as reference while writing tests" | You'll adapt the reference. That's testing after with extra steps. |
| "TDD is too dogmatic" | TDD IS the pragmatic approach — it finds bugs before commit. |
| "Just this once" | Every violation is "just this once." The rule exists for every time. |
| "UI code can't be unit tested" | Separate logic from presentation. Test the logic layer. Snapshot test the presentation. If you can't, the UI has a design problem. |

## Red Flags — STOP and Reconsider

If you notice any of these, you are about to violate TDD. Stop. Go back to RED.

- You are writing production code and haven't run a failing test yet
- You are writing a test and already know exactly what the implementation looks like
- You are "just adding a small thing" without a test
- You are thinking "I'll add the test in a minute"
- You are fixing a test to match unexpected implementation behavior
- You are mocking your own internal classes to make a test work
- You are writing multiple tests before any implementation
- You are building UI with logic embedded in the presentation layer instead of a testable abstraction

## When to Use TDD

**Always use for**: new features, bug fixes, refactoring behavior, business logic, API endpoints, state management, data transformations, algorithms, UI logic and presentation.

**UI is not an exception.** Separate presentation from logic. Use MVC, MVVM, Presenter/Container, hooks, or any pattern that gives you a unit-testable layer beneath the UI. The logic and state layer gets full TDD. The presentation layer gets snapshot tests. "UI is hard to test" means the UI has a design problem — fix the design, don't skip the tests.

**Ask your human partner about**: throwaway prototypes, generated code, configuration files, database migrations.

## Announcing TDD Usage

When starting TDD work, state what you're doing:

> "I'll implement [feature] using TDD. Starting with a failing test for [first behavior]."

After each cycle, briefly report:

> "RED: test for [behavior] fails as expected. Writing minimal implementation."
> "GREEN: all tests pass. Refactoring [what]."

This creates commitment and makes the process visible.

## Working with Your Human Partner

You and your partner share the goal of correct, well-tested code. Before starting:

- Confirm which behaviors to test (you can't test everything — prioritize)
- Confirm the public interface design
- Ask if any behaviors need 100% coverage

During work, communicate what cycle you're in. If you're tempted to skip TDD, say so — your partner can help you stay on track or make an informed exception.
