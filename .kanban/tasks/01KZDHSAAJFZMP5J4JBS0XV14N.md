---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzdpw3th21g8qdjeb1b8cmmk
  text: |-
    Research (measured, not guessed). A temporary integration test dumped the labelled JS/TS tree for `it(...)`, `test(...)`, `it.skip(...)`, `xit(...)`, `describe(...)`, and `describe.each(...)`. The measurements, all deleted with the probe before finishing:

    - A call is `call_expression` with a `function:` field and an `arguments:` field.
    - The callee is an `identifier` for `it`/`test`/`xit`, a `member_expression` for `it.skip`/`test.skip` (text reads `it.skip`), and a nested `call_expression` for `describe.each([1])(...)`.
    - The test body is NOT the call. It hangs on the `arrow_function` (or `function_expression`) the call takes as its second argument, which sits directly inside `arguments`, so the call is exactly two levels up from the callback.
    - A `string` argument keeps its quotes in its own text; the unquoted text is a `string_fragment` child. JS accepts single quotes, so quote stripping had to cover both.
    - `catch_clause` is the catch kind; `expect` is an `identifier`, chai's `should` is a `property_identifier`.

    Design consequence. Making the CALL the function node fails: `arrow_function` is already a JS `function_kind`, so the callback would still be scored separately and NOT exempt, which is exactly the gap the card is about. The callback therefore had to stay the definition, with the marker read from the call one level out.

    Dead end avoided: `describe` deliberately stays OFF `call_target_test_kinds`. A suite asserts nothing, so marking its callback a test would both exempt real code from the complexity gates and make the census accuse an honest suite of measuring nothing.
  timestamp: 2026-08-07T08:54:11.025654+00:00
- actor: claude-code
  id: 01kzdpwjzt2m389axkfwjdf6rj
  text: |-
    Implementation landed.

    `complexity.rs`
    - `call_target_text` is no longer Elixir-shaped. Three new `ComplexitySpec` rows supply the shape: `call_kind` (Elixir `call`, JS family `call_expression`), `callee_field` (`target` / `function`), and `callee_kinds` (Elixir `identifier` only so `Mod.fun()` stays an ordinary call; the JS family adds `member_expression` so `it.skip` reads).
    - New `defining_call` returns the call that defines a function plus its callee text, covering both shapes: the definition IS the call (Elixir), or the definition is the call's callback argument (`test_callback_kinds`, the JS family's `arrow_function`/`function_expression`, reached two levels up). It answers `None` for a call whose callee names no definition, so `arr.map(v => v + 1)` keeps its own identity.
    - `is_test_definition` and `function_name` both read that ONE lookup. `name_from_call_arguments` is deleted — the bool was the "is this a call definition?" question `defining_call` now answers, so Elixir keeps its behaviour with one less field.
    - `function_expression` joins the JS `function_kinds`. Without it mocha's `it("...", function () {})` would produce no census row at all, which reads as clean — the precise failure the module exists to prevent.
    - `STRING_QUOTES` names the quote pair stripped off a description string, so single-quoted `it('...')` names its row too.

    `test_census.rs`
    - `javascript_family_census` adds three rows (javascript, typescript, tsx): `expect`/`assert`/`should` words, `skip`/`xit`/`xtest` markers, `catch_clause`, and `property_identifier` beside `identifier` so chai's `should` reads.
    - `is_skipped` now also matches the defining call's callee against `skip_markers`, which is how `it.skip`/`test.skip`/`xit` are measured as skipped — they carry the marker in the name they are called by rather than in an annotation.

    Moved probe test off JavaScript: `tree_sitter_probes.rs::a_language_with_no_census_mapping_reports_one_not_computed_row` and the sibling in `test_census.rs` now use Elixir (`thing_test.exs`). The assertion is not weakened — it is stronger: Elixir's `test "..." do` IS a recognized test definition, so ONLY the missing vocabulary stands between that file and a clean result.
  timestamp: 2026-08-07T08:54:26.554252+00:00
- actor: claude-code
  id: 01kzdpww9xrvfgj3sb4axwqzr9
  text: |-
    ### implement — changed
    - evidence: 3 files — crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs, crates/swissarmyhammer-sem/src/parser/plugins/code/complexity/test_census.rs, crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs. TDD: 6 new tests written first and watched fail (5 panicked "the sample's language has a census mapping", 1 read `[("<anonymous>", false), ("<anonymous>", false)]`), then green. `cargo test -p swissarmyhammer-sem -p swissarmyhammer-validators` 333 + 482 + 2 + 2 passed, 0 failed. `cargo fmt --all -- --check` clean, `cargo clippy --all-targets` 0 warnings, `cargo check --workspace --all-targets` clean. Temporary tree-dumping probe deleted.
    - next: /review
  timestamp: 2026-08-07T08:54:36.093832+00:00
position_column: doing
position_ordinal: '8480'
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