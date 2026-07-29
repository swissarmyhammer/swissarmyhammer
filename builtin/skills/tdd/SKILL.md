---
name: tdd
description: Use this skill before you write or change production code. It enforces strict test-driven development (RED, GREEN, REFACTOR). Write the failing test first. Watch it fail. Then write the code to pass. Use it when the user says "tdd", "test first", "write the test first", "red-green-refactor", "write a failing test", or when you implement a new function, fix a bug, or add behavior that needs a regression test. Do not use it to read, explore, or explain existing code. Use the explore skill instead. Do not use it to run a test suite that already exists. Use the test skill instead. Do not use it for a pure refactor that adds no new behavior and keeps the existing tests green.
license: MIT OR Apache-2.0
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Test-Driven Development (TDD)

Write the test first. Watch it fail. Write correct, well-designed code to pass.

**Core principle:** if you did not watch the test fail, you do not know whether it tests the right thing.

**Optimize for correctness, not speed.** Breaking the letter of the rules breaks the spirit of the rules too.

## When to Use

Use it for all code changes. There are no exceptions. If work is worth coding, it is worth testing. Do you think, "I will skip it just this once"? That is rationalization, not a reason.

## The Iron Law

```
NO PRODUCTION CODE WITHOUT A FAILING TEST FIRST
```

Did you write code before the test? **Delete it.** Do not keep it as a "reference". Do not "adapt" it. Do not look at it again. Delete means delete.

## Red-Green-Refactor

### RED — write the failing test

Write one minimal test. It shows the intended behavior:
- Test one behavior in each test. If the test name has "and" in it, split the test.
- Give the test a clear, descriptive name.
- Use real code, not mocks, unless you cannot avoid a mock.
- Show the intended API.

### Verify RED — watch it fail (mandatory)

Run the test. Confirm that:
- It **fails**. It does not error, and it does not fail to compile.
- The failure message matches what you expect.
- It fails because the feature is missing, not because of a typing error.

**Does the test pass immediately?** Then you are testing existing behavior. Fix the test.
**Does the test error?** Fix the error. Run the test again until it fails correctly.

### GREEN — correct code

Write correct, well-designed code. The code must pass the test and follow the patterns of the codebase.
- Add no feature beyond what the test requires.
- Do not do unrelated refactoring here. That step is REFACTOR.
- Match the existing style, idioms, and conventions.

### Verify GREEN — watch it pass (mandatory)

Run the test. Confirm that:
- The new test passes.
- All other tests still pass.
- The output is clean. There are no errors and no warnings.

**Does the new test fail?** Fix the code, not the test.
**Did other tests break?** Fix them now.

### REFACTOR — clean up

Do this only after GREEN:
- Remove duplication.
- Improve names and clarity.
- Extract helper functions that follow existing patterns.
- Make the solution robust and idiomatic.

Keep the tests green throughout this step. Do not add new behavior. Strengthen the existing behavior only.

### Repeat

Write the next failing test for the next behavior.

## Rationalizations vs Reality

| Excuse | Reality |
|--------|---------|
| "Too simple to test" | Simple code can break. A test takes about 30 seconds. |
| "I'll test after" | Tests written after the code prove nothing. They pass immediately. |
| "Tests-after same goal" | Testing after asks "what does this do?" Testing first asks "what should this do?" These are different goals. |
| "Already manually tested" | Manual testing is not systematic testing. You cannot run it again. |
| "Deleting X hours is waste" | This is the sunk-cost trap. Unverified code is technical debt. |
| "Keep as reference, write tests" | You will adapt the old code. That is testing after. Delete the code. |
| "Need to explore first" | This is fine. Throw away the exploration code. Then start TDD. |
| "Test hard = design unclear" | Listen to the test. Code that is hard to test is hard to use. |
| "TDD slows me down" | TDD is faster than debugging. |
| "Manual is faster" | Manual testing misses edge cases. You must re-test after every change. |
| "Existing code has no tests" | Add tests for the code you touch. |

## Red Flags — STOP and Start Over

- You wrote code before the test.
- You added the test after the code.
- The test passes on first run.
- You cannot explain why the test failed.
- Tests "added later"
- "Just this once", "I already tested this manually", or "this is the spirit, not the ritual"
- "Keep it as a reference" or "adapt the existing code"
- You spent hours already, so deleting feels wasteful.
- You call the rule "dogmatic" instead of "pragmatic", or you say "this is different because..."

Each of these signs means: delete the code. Start over with TDD.

## Why Order Matters

**A test written after the code passes immediately.** Passing immediately proves nothing. The test might check the wrong thing. The test might check the implementation, not the behavior. The test might miss edge cases. You never saw the test catch a bug. Testing first forces you to watch the test fail. This proves that the test checks something real.

**Sunk cost is the wrong way to think about this.** The time is gone either way. You have a choice. You can delete the code and use TDD: this takes more hours, but gives high confidence. Or you can keep the code and add tests after: this takes about 30 minutes, but gives low confidence and likely bugs. The real waste is keeping code you cannot trust.

**TDD is a practical method.** It finds bugs before you commit. It prevents regressions. It documents behavior. It supports safe refactoring. A "practical" shortcut often leads to debugging in production, which is slower.

## Good Tests

| Quality | Good | Bad |
|---------|------|-----|
| Minimal | One thing | `test('validates email and domain and whitespace')` |
| Clear | Name describes behavior | `test('test1')` |
| Shows intent | Shows the wanted API | Hides what the code should do |
| Real code | Tests actual behavior | Tests mock behavior |

## When Stuck

| Problem | Solution |
|---------|----------|
| Do not know how to test it | Write the wanted API first. Write the assertion first. Ask the user. |
| The test is too complicated | The design is too complicated. Simplify the interface. |
| You must mock everything | The code is too tightly coupled. Use dependency injection. |
| Test setup is large | Extract helper functions. If it is still complex, simplify the design. |

## Bug Fixes

Write a failing test that reproduces the bug. Then follow the red-green-refactor cycle. The test proves the fix and prevents the bug from returning. **Do not fix a bug without a test.**

## Cover the Inverse and the Siblings

A fix that passes *your* test can still be incomplete. Before GREEN, ask these two questions:

- **Did you change one side of a pair, such as write and read, serialize and deserialize, encode and decode, classify and parse, or set and get?** If so, test the **inverse direction** too. Round-trip the data. A test *named* "round-trip" must actually read back what it wrote. A write-only test that checks an output string is not a round-trip test, no matter what its docstring says.
- **Did you change how the code handles one token, flag, case, or format?** Then search for every other place that uses the same value, and test those places too. A common mistake: you make a classifier case-insensitive, but a value-parser elsewhere still compares text as case-sensitive.

The real, hidden test almost always checks the matching or sibling path that you did not think of. It is rarely the exact example from the issue. A test written only from your own mental model will pass by design. It proves nothing about the part you forgot.

## Verification Checklist

Before marking complete:
- [ ] Every new function or method has a test
- [ ] You watched each test fail before you wrote the code
- [ ] Each test failed for the expected reason: the feature was missing, not because of a typing error
- [ ] You wrote correct, well-designed code to pass the test
- [ ] All tests pass
- [ ] Output is clean: no errors, no warnings
- [ ] You used real code; mocks only when you could not avoid them
- [ ] Edge cases and errors are covered
- [ ] The inverse direction, or round-trip, is covered when you changed one side of a pair, such as write and read, encode and decode, or classify and parse
- [ ] You ran the **existing** test module for the code you touched, not only your new tests

Can you not check every box? Then you skipped TDD. Start over.

## Final Rule

```
Production code → test exists and failed first
Otherwise → not TDD
```

There are no exceptions without clear permission from the user.
