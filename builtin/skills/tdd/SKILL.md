---
name: tdd
description: Use before writing or changing production code — enforces strict test-driven development (RED, GREEN, REFACTOR) by writing the failing test first, watching it fail, then writing the code to pass. Use when the user says "tdd", "test first", "write the test first", "red-green-refactor", "write a failing test", or when implementing a new function, fixing a bug, or adding behavior that needs a regression test. Do NOT use for reading, exploring, or explaining existing code — use the explore skill instead. Do NOT use for running an already-written test suite — use the test skill. Do NOT use for pure refactors that add no new behavior and keep the existing tests green.
license: MIT OR Apache-2.0
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Test-Driven Development (TDD)

Write the test first. Watch it fail. Write correct, well-designed code to pass.

**Core principle:** if you didn't watch the test fail, you don't know if it tests the right thing.

**Optimize for correctness, not speed.** Violating the letter of the rules violates the spirit.

## When to Use

All code changes, no exceptions. If it's worth coding, it's worth testing. Thinking "skip just this once"? That's rationalization.

## The Iron Law

```
NO PRODUCTION CODE WITHOUT A FAILING TEST FIRST
```

Wrote code before the test? **Delete it.** Don't keep as "reference", don't "adapt", don't look at it. Delete means delete.

## Red-Green-Refactor

### RED — write the failing test

One minimal test showing intended behavior:
- One behavior per test ("and" in the name? split it)
- Clear, descriptive name
- Real code, not mocks (unless unavoidable)
- Demonstrates intended API

### Verify RED — watch it fail (mandatory)

Run it. Confirm:
- It **fails** (not errors, not compile-fails)
- Failure message matches expectation
- Fails because the feature is missing, not because of typos

**Passes immediately?** You're testing existing behavior — fix the test.
**Errors?** Fix the error, re-run until it fails correctly.

### GREEN — correct code

Write correct, well-designed code that passes and follows the codebase's patterns.
- No features beyond what the test requires
- No unrelated refactors here — that's REFACTOR
- Match existing style, idioms, conventions

### Verify GREEN — watch it pass (mandatory)

Run it. Confirm:
- New test passes
- Other tests still pass
- Output is pristine — no errors, no warnings

**New test fails?** Fix the code, not the test.
**Other tests broke?** Fix them now.

### REFACTOR — clean up

Only after green:
- Remove duplication
- Improve names and clarity
- Extract helpers following existing patterns
- Make the solution robust and idiomatic

Keep tests green throughout. No new behavior; harden existing.

### Repeat

Next failing test for the next behavior.

## Rationalizations vs Reality

| Excuse | Reality |
|--------|---------|
| "Too simple to test" | Simple code breaks. Tests take 30s. |
| "I'll test after" | Tests-after prove nothing — they pass immediately. |
| "Tests-after same goal" | After = "what does this do?"; first = "what should this do?" |
| "Already manually tested" | Ad-hoc ≠ systematic; can't re-run. |
| "Deleting X hours is waste" | Sunk cost. Unverified code is tech debt. |
| "Keep as reference, write tests" | You'll adapt it — that's testing after. Delete. |
| "Need to explore first" | Fine — throw away the exploration, start TDD. |
| "Test hard = design unclear" | Listen to the test. Hard to test = hard to use. |
| "TDD slows me down" | TDD is faster than debugging. |
| "Manual is faster" | Manual misses edge cases; you re-test every change. |
| "Existing code has no tests" | Add tests for what you touch. |

## Red Flags — STOP and Start Over

- Code written before test
- Test added after implementation
- Test passes on first run
- Can't explain why the test failed
- Tests "added later"
- "Just this once" / "already manually tested" / "spirit not ritual"
- "Keep as reference" / "adapt existing"
- "Sunk hours, deletion wasteful"
- "Dogmatic vs pragmatic" / "this is different because..."

All of these = delete the code, start over with TDD.

## Why Order Matters

**Tests written after code pass immediately.** Passing immediately proves nothing — might test the wrong thing, might test implementation not behavior, might miss edge cases, you never saw it catch a bug. Test-first forces you to see the test fail, proving it tests something.

**Sunk cost is the wrong frame.** Time is gone either way. Choice: delete + TDD (more hours, high confidence) or keep + tests after (30 min, low confidence, likely bugs). The waste is keeping untrusted code.

**TDD is pragmatic.** Finds bugs before commit, prevents regressions, documents behavior, enables refactoring. "Pragmatic" shortcuts = production debugging = slower.

## Good Tests

| Quality | Good | Bad |
|---------|------|-----|
| Minimal | One thing | `test('validates email and domain and whitespace')` |
| Clear | Name describes behavior | `test('test1')` |
| Shows intent | Demonstrates the desired API | Obscures what the code should do |
| Real code | Tests actual behavior | Tests mock behavior |

## When Stuck

| Problem | Solution |
|---------|----------|
| Don't know how to test | Write the wished-for API; write the assertion first; ask the user. |
| Test too complicated | Design too complicated — simplify the interface. |
| Must mock everything | Code too coupled — use DI. |
| Test setup huge | Extract helpers; still complex → simplify design. |

## Bug Fixes

Write a failing test that reproduces the bug, then follow the cycle. The test proves the fix and prevents regression. **Never fix bugs without a test.**

## Verification Checklist

Before marking complete:
- [ ] Every new function/method has a test
- [ ] Watched each test fail before implementing
- [ ] Each failed for the expected reason (feature missing, not typo)
- [ ] Wrote correct, well-designed code to pass
- [ ] All tests pass
- [ ] Output pristine (no errors, no warnings)
- [ ] Real code (mocks only if unavoidable)
- [ ] Edge cases and errors covered

Can't check all? You skipped TDD. Start over.

## Final Rule

```
Production code → test exists and failed first
Otherwise → not TDD
```

No exceptions without explicit user permission.
