---
assignees:
- claude-code
depends_on:
- 01KW25ZK93AJ0CR17C9QJ11RXW
- 01KW263S53NJ1YWNHGPYTTWEEC
position_column: todo
position_ordinal: b080
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