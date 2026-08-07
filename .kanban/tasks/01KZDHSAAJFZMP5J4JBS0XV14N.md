---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8d80
title: Detect jest/mocha `it(...)` and `test(...)` as test definitions
---
The JavaScript/TypeScript/TSX `ComplexitySpec` rows in `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` mark a test only from a `@Test` decorator. Real JS and TS tests are a call — `it("...", () => {...})`, `test("...", async () => {...})`, `describe.each(...)` — so no JS or TS test is recognized as one today.

`call_target_text` already reads a call-based definition, but it is Elixir-shaped: it requires `node.kind() == "call"` and a bare `identifier` target. The JS grammar spells the same shape `call_expression` with a `function` field, and the test body is an arrow function argument rather than the call itself.

Two probes are blind to the JS family because of this:
- `complexity` never exempts a jest test from its gates.
- `assertion-census` cannot map the JS family at all (found while building it, task ^cysg4xv); a mapping would report a file full of untested tests as clean, so the family is deliberately left out of `TEST_CENSUS_SPECS`.

Work:
- Generalize the call-based definition lookup to read the callee by grammar-supplied field, and add the JS/TS/TSX rows naming `it`/`test` as test call targets.
- Add the JS family rows to `TEST_CENSUS_SPECS`: the `expect`/`assert`/`should` words, `it.skip`/`xit`/`test.skip` as skips, `catch_clause` as the catch kind.
- Tests: `it("...", () => { expect(x).toBe(1); })` measures no defect; `it("...", () => {})` yields a row; `it.skip(...)` yields a row; a bare helper function beside them is not a test.

#tool-validators