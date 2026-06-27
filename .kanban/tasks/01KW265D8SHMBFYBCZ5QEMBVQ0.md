---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw3f1dz7xa14qjvmfbee20e7
  text: |-
    Implemented assertion compiler in crates/swissarmyhammer-expect/src/assertion.rs (compile + CompiledAssertion/Locator/Expected/BoundValue/AssertOp/Stream/CompileError/AssertionOutcome), wired into lib.rs, added regex dep. All 4 acceptance criteria covered with TDD (verified RED via breaking numbers_equal: 9 assertion tests fail; reverted to GREEN). 104 nextest pass, 5 doc tests pass, clippy -D warnings clean, fmt clean.

    Adversarial double-check returned REVISE with 4 advisory findings. Addressing all:
    1. self-verify gate untested on its load-bearing path (invariant where left != count) -> adding test.
    2. find_scalar_path_by_value value-only fallback could silently bind an unrelated field when a hint-named field exists with a different value (contradicts "never a silent mis-read") -> hardening to reject the mis-read while preserving unanchored-hint value binding.
    3. classify committed to exit intent greedily with no fallthrough -> restructuring compile to try recognized kinds (exit > invariant > literal) and return the first that binds+self-verifies.
    4. no CheckpointOutOfRange test -> adding.
  timestamp: 2026-06-27T02:37:46.855456+00:00
- actor: claude-code
  id: 01kw3fa4ke1tnkgrrvjf1pcf5z
  text: |-
    All 4 double-check findings resolved; re-review returned VERDICT: PASS (independently verified 108 nextest + 5 doctests). Final state green:
    - cargo nextest run -p swissarmyhammer-expect: 108 passed (16 in assertion::tests)
    - cargo test --doc -p swissarmyhammer-expect: 5 passed
    - cargo clippy -p swissarmyhammer-expect --all-targets -- -D warnings: clean
    - cargo fmt --check: clean

    Files: crates/swissarmyhammer-expect/src/assertion.rs (new), src/lib.rs (module+re-exports), Cargo.toml (regex dep). Leaving task in `doing` for /review.
  timestamp: 2026-06-27T02:42:32.174301+00:00
depends_on:
- 01KW25ZK93AJ0CR17C9QJ11RXW
- 01KW263S53NJ1YWNHGPYTTWEEC
position_column: doing
position_ordinal: '8280'
project: expect
title: 'Assertion compiler + cli locators (Tier 1: literal + invariant)'
---
## What
Compile each `Then` criterion into a typed, replayable assertion bound to a checkpoint + locator, choosing the cheapest faithful tier. cli locator dialect + Tier 1 kinds only here. Per `ideas/expect.md` §"How evaluate turns prose into a check".

- New `crates/swissarmyhammer-expect/src/assertion.rs`:
  - `CompiledAssertion { checkpoint: usize, locator: Locator, op: AssertOp, expected: Expected, tier: VerdictTier, criterion_text: String }`.
  - `Locator` — cli dialect: stream regex-capture, json-path (if output is JSON), `exit`. Compiler prefers the most durable locator that binds (json-path over text-regex).
  - Two deterministic flavors: **literal-match** (`$.total equals 40`) and **invariant-holds** (relationship derived from the observation each run, e.g. `for each X: a == count(b)`). Prefer invariants where expressible (the authoring skill pushes for them).
  - `compile(criterion, observation) -> Result<CompiledAssertion, CompileError>`: binds against a REAL observation (you can't write `$.total` without seeing output shape). **Self-verifying**: a freshly compiled assertion MUST bind and pass against the very observation it was compiled from, else reject as a hallucinated locator.
  - A locator that stops binding later is structural drift (surfaced loudly), never a silent mis-read — represent that as a distinct outcome.
- Tier selection by assertion KIND (locator + exact/regex/numeric ⇒ Tier 1). Tiers 2/3 are stubs/markers here; their evaluation lands in later tasks.

## Acceptance Criteria
- [ ] "after the first apply, the total is $40" compiles to `{checkpoint:1, locate:<json-path or regex>, op:equals, expected:40.00, tier:Deterministic}` against a matching observation.
- [ ] An invariant criterion compiles to invariant-holds with the expected derived from the observation, not a frozen literal.
- [ ] A criterion whose locator does not bind/pass against its source observation is REJECTED at compile time (hallucinated-locator guard).
- [ ] json-path is preferred over text-regex when output is JSON.

## Tests
- [ ] `crates/swissarmyhammer-expect/src/assertion.rs` unit tests: literal compile, invariant compile, self-verification rejection, json-path-vs-regex preference. Use fixture Observations.
- [ ] `cargo nextest run -p swissarmyhammer-expect assertion` passes.

## Workflow
- Use `/tdd`.