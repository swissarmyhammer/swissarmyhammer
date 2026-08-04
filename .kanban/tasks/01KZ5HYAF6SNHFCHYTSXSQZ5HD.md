---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz6ecw3yvy2rsry6wp7w2v9q
  text: |-
    ### implement — changed

    - evidence: 2 files — `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` (+2004/-93), `crates/swissarmyhammer-validators/src/review/probes.rs` (+9/-4)
    - Grammar-verified (real `Node::to_sexp()`/labelled-tree output, never assumed) and mapped: go, ruby, fortran, swift, elixir — each with its own `ComplexitySpec` row and a 6-test suite (match/switch flat, if/elif/else flat, nested-deepens, boolean run once/mixed twice, test-marker exempts, determinism). Fortran has an extra 3-way-chain-equivalent flat-scoring proof folded into its if/elseif/else test; Swift additionally gets `swift_three_way_else_if_chain_increments_at_each_level` proving the marker-based walker increments correctly at each of 3 levels (score 4, chain 2, depth still 1).
    - New generic mechanisms added to `ComplexitySpec` (all data-driven, zero per-language branches in the walker):
      - `test_name_prefix`/`test_param_type`/`parameters_field` + `name_signature_marks_test` — go/ruby/fortran's name+signature test convention.
      - `header_child_kinds` + `function_header` — fortran's field-owning child indirection (`subroutine`/`function` wrap a `subroutine_statement`/`function_statement` that owns `name`/`parameters`).
      - `condition_field` — generalized the previously-hardcoded `"condition"` literal (elixir needs `"arguments"`).
      - `else_marker_kinds` + `walk_marker_conditional` — swift's positional, `else`-marker-delimited consequence/alternative.
      - `positional_conditional`/`statement_terminator_kinds` + `walk_positional_conditional` — fortran's fully fieldless conditional.
      - `call_target_kinds`/`call_target_test_kinds`/`name_from_call_arguments` + `effective_kind`/`call_target_text`/`call_function_name` — elixir's call-classified special forms.
      - `alternative_nested_in_consequence` + `walk_consequence_with_nested_alternative` — elixir's alternative nested inside the SAME consequence container.
      - `child_by_field_or_kind` — elixir's `call` node declares only a `target` FIELD (verified via `node-types.json`); `arguments`/`do_block` are positional children whose KIND is spelled the same as the concept.
    - Deeper finding beyond the task's original text: Elixir's OWN `if`/`unless`/`case`/`cond` are ALSO `call` nodes distinguished only by target text, not just `def`/`defp`/`defmacro` as the task described. Solved generically via `effective_kind`, applied everywhere a node's kind is checked (function detection, nesting, conditionals, arms) rather than narrowly patching function detection. Verified no false positives on ordinary calls (`elixir_an_ordinary_call_is_never_mistaken_for_a_function_or_conditional`).
    - What did NOT work initially, for the next agent's benefit: my first elixir pass used `child_by_field_name("arguments")`/`("do_block")`, based on misreading my own scratch dumper's output (it printed the CHILD'S OWN KIND as if it were a field label when `field_name_for_named_child` returned `None`). Root-caused by comparing `field_name_for_named_child` against `child_by_field_name` directly on a live parsed node — elixir's `call` node declares only `target` as a real field per `node-types.json`; `arguments`/`do_block` must be found by kind, not by field. Fixed via `child_by_field_or_kind`.
    - Deferred: bash. Independently re-verified (real parse of a bats-core `# @test "..."` sample): the comment is a plain, unstructured `comment` sibling of `function_definition`, indistinguishable by KIND from an ordinary doc comment or license header — unsafe to treat as a marker, matching the task's own reasoning. No safe name+signature convention exists for bash either. Left unmapped; reports not-computed via the pre-existing `COMPLEXITY_NOT_COMPUTED` mechanism in `probes.rs` (no duplicate mechanism added). Updated `complexity_reports_not_computed_for_an_unmapped_language` to use bash (`src/app.sh`) instead of go, since go is now mapped.
    - Elixir has no imperative loop construct at all (`for` is a functional comprehension, itself a `call`; recursion is idiomatic) — `loop_kinds` is intentionally empty and documented; its "nested loops deepen" test uses nested conditionals instead and asserts `max_loop_nesting == 0`.
    - Verification: `cargo fmt --all` clean. `cargo clippy -p swissarmyhammer-sem --all-targets -- -D warnings` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean. `cargo nextest run -p swissarmyhammer-sem` — 295 passed, 0 failed, 0 skipped. `cargo nextest run -E 'rdeps(swissarmyhammer-sem)'` — 3374 passed, 0 failed, 0 skipped (run twice, before and after a doc-only follow-up edit).
    - Scratch grammar-verification harness (`tests/scratch_grammar_dump.rs`) was written, used to verify every node kind/field against the real compiled grammars for all 6 languages, then deleted — not part of the final diff (confirmed via `git status`, only the two intended files are modified).

    next: /review
  timestamp: 2026-08-04T13:11:21.982842+00:00
- actor: claude-code
  id: 01kz6eprkv6ycywths2m47h2vv
  text: |-
    ### finish iteration 1 — changed
    - implement: changed — grammar-verified ComplexitySpec mapping for go/ruby/fortran/swift/elixir, new generic mechanisms (name+signature test-marking, marker-conditional walker, positional-conditional walker, call-target filtering + effective_kind), bash deferred as genuinely unmappable with verified reasoning
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-sem)', 3374 passed, 0 failed, 0 skipped; per-language 6-test suites confirmed present
    - commit: 7764fa05f
    - review: pending
  timestamp: 2026-08-04T13:16:46.075187+00:00
- actor: claude-code
  id: 01kz6fmacmrjd65j59rw6h7h74
  text: |-
    ### review — findings
    - evidence: 3 findings — crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs:396, crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs:864, crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs:1444
    - scope: review sha HEAD~1..HEAD (7764fa05f)
    - next: extract shared ComplexitySpec default-field helper; add uppercase Fortran logical-operator test; make Fortran test-name-prefix matching case-insensitive
  timestamp: 2026-08-04T13:32:54.548827+00:00
- actor: claude-code
  id: 01kz6fmy3bx0zjycb0n9zw4032
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — grammar-verified ComplexitySpec mapping for go/ruby/fortran/swift/elixir, new generic mechanisms, bash deferred
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-sem)', 3374 passed, 0 failed, 0 skipped
    - commit: 7764fa05f
    - review: findings — crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs:396 (repeated default field-init block across 7+ ComplexitySpec definitions, extract helper), :864 (fortran logical_operators only lowercase .and./.or., no uppercase test/handling), :1444 (fortran test-name-prefix detection is case-sensitive starts_with, misses uppercase names)
  timestamp: 2026-08-04T13:33:14.731110+00:00
- actor: claude-code
  id: 01kz6gk6ezt72dtr3gezssqpvb
  text: |-
    ### implement — changed

    Fixed all 3 review findings in `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs`:

    1. **Dedup (was line ~396).** Added `const EXTENDED_SPEC_DEFAULTS: ComplexitySpec` holding the defaults for the 12 fields ^xjyb2qf added (`test_name_prefix` through `name_from_call_arguments`, plus the new `test_name_case_insensitive` field below). Every spec literal now ends with `..EXTENDED_SPEC_DEFAULTS` and only sets the fields that actually differ, applied to ALL 15 spec definitions (RUST_SPEC, `typescript_family_spec`, PYTHON_SPEC, JAVA_SPEC, `c_family_spec`, CSHARP_SPEC, PHP_SPEC, GO_SPEC, RUBY_SPEC, FORTRAN_SPEC, SWIFT_SPEC, ELIXIR_SPEC) — not just the 7 that needed zero overrides.

    2. **Fortran uppercase `.AND.`/`.OR.` (was line ~864).** Verified against `tree_sitter_fortran` 0.6.0's own `grammar.js`: `logical_expression`'s `.and.`/`.or.` tokens are built with `caseInsensitive('.and.')`/`caseInsensitive('.or.')`, which aliases the token to the SAME lowercase node kind regardless of source casing (a regex `[aA][nN][dD]` etc., aliased to the literal `.and.`). So `node.kind()` for an uppercase `.AND.` in real parsed source is still `".and."` — no code change needed. Added `fortran_boolean_operators_are_recognized_regardless_of_case`, which parses real uppercase `.AND.`/`.OR.` Fortran source and asserts identical `cognitive_score`/`max_boolean_operands` to the existing lowercase test. Ran and confirmed PASS (proof by real parse, not just reading the grammar source).

    3. **Fortran uppercase test-name prefix (was line ~1444).** This WAS a real bug: `name_signature_marks_test`'s `name.starts_with(prefix)` is case-sensitive, and unlike the grammar's own tokens, Fortran identifier TEXT is never case-normalized by the grammar (it's arbitrary user text). Added a new `ComplexitySpec::test_name_case_insensitive: bool` field (default `false` via `EXTENDED_SPEC_DEFAULTS`, set `true` only for `FORTRAN_SPEC`) rather than making the match case-insensitive for all 3 name+signature languages — Go and Ruby are case-sensitive languages (`go test` itself requires the exact-case `Test` prefix), so a blanket case-insensitive fix would have created false positives there. TDD: added `fortran_uppercase_test_name_prefix_exempts_the_subroutine` first, ran it, confirmed RED (failed on `scored.is_test` exactly as expected, not a compile error), then implemented the `test_name_case_insensitive` branch in `name_signature_marks_test` using `name.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix))` (bounds-checked, no panic risk on short names), ran again, confirmed GREEN.

    No scratch grammar-dump file was needed this time — `tree-sitter-fortran`'s `grammar.js` (read directly from the cargo registry checkout) gave conclusive static evidence for finding 2, and the new permanent tests themselves are the real-parse verification for both findings 2 and 3.

    Verification:
    - `cargo fmt --all` — clean, no diff beyond the intended edits.
    - `cargo clippy -p swissarmyhammer-sem --all-targets -- -D warnings` — clean.
    - `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` — clean.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-sem)'` — 3376 tests run: 3376 passed, 0 failed, 0 skipped.
    - `cargo nextest run -p swissarmyhammer-sem -E 'test(fortran)'` — 8 passed including both new tests.

    next: /review
  timestamp: 2026-08-04T13:49:46.335135+00:00
- actor: claude-code
  id: 01kz6gswnk16jhs7q2hhptjpj7
  text: |-
    ### finish iteration 2 — changed
    - implement: changed — extracted EXTENDED_SPEC_DEFAULTS shared across 15 ComplexitySpec definitions; proved fortran .and./.or. already case-insensitive at grammar level; fixed genuine case-sensitivity bug in fortran test-name-prefix detection via new test_name_case_insensitive field
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-sem)', 3376 passed, 0 failed, 0 skipped
    - commit: 12b31a900
    - review: pending
  timestamp: 2026-08-04T13:53:25.683074+00:00
- actor: claude-code
  id: 01kz6h1rws1hwv9tkz82rzw82c
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (12b31a900) — 0 findings (2 refuted, 9 attempted, 0 failed, 0 skipped); all 3 prior findings already checked
    - next: task moved to done
  timestamp: 2026-08-04T13:57:43.961354+00:00
- actor: claude-code
  id: 01kz6h2cwajzydpxjm46ashx12
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — extracted EXTENDED_SPEC_DEFAULTS shared across 15 ComplexitySpec definitions; fixed fortran test-name-prefix case-sensitivity bug
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-sem)', 3376 passed, 0 failed, 0 skipped
    - commit: 12b31a900
    - review: clean — task moved to done
  timestamp: 2026-08-04T13:58:04.426551+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffa380
title: 'complexity scorer: map go, ruby, fortran, swift, elixir, bash (needs design extensions)'
---
This task follows ^xjyb2qf ("complexity scorer covers only Rust — map the remaining source_code languages"). That task added ComplexitySpec rows for typescript, tsx, javascript, python, java, c, cpp, csharp, and php — all grammar-verified with the full 6-test suite. Six languages from the original scope remain unmapped, each blocked on a real, specific gap rather than missing effort:

## go

No attribute/annotation grammar construct exists in `tree_sitter_go` for test-marking (verified: parsing a function has no decorator/annotation node kind at all). Go's real test convention is name+parameter-type based (`func TestXxx(t *testing.T)`), a fundamentally different mechanism from the attribute-based `is_test_definition`/`attribute_marks_test` machinery every mapped language shares. Needs a new, generic (not per-language-branched) spec mechanism — e.g. a `test_name_prefix`/`test_param_type` pair — before Go can share the same 6-test suite honestly.

## ruby

Same class of gap as Go: `tree_sitter_ruby` has no attribute/annotation node kind. RSpec/minitest mark tests via a call-based DSL (`it "does x" do ... end`, not a `def`) or a naming convention (`def test_foo`), neither of which is an attachable attribute node the existing mechanism can check.

## fortran

No attribute/annotation mechanism exists in `tree_sitter_fortran` at all (verified while mapping its `if`/`do`/`select case` node kinds — no such node kind appeared). No idiomatic test-marking convention exists for Fortran either, so there is no realistic sample to write the required "test marker exempts the function" test against.

## swift

Swift DOES have a genuine, current test-marking mechanism — verified: `@Test` parses as `modifiers > attribute`, matching the real Swift Testing framework (Swift 6). It is blocked on something else: its `if`/`else if`/`else` shape has NO wrapping `else_clause` node at all. Verified via `tree_sitter_swift`: the nested `if_statement` (or the final body) sits as an EXTRA direct child of the SAME outer `if_statement`, following an anonymous `else` token — not through a single or repeated `alternative` field the way every one of the 9 newly-mapped languages does. ^xjyb2qf's `walk_conditional`/`walk_alternative` design is built entirely around `child_by_field_name("alternative")` (single, recursive) or a repeated `alternative` field (Python/PHP's flat elif model); Swift fits neither shape and needs its own structural handling before it can be added without special-casing the shared walker.

## elixir

Functions are represented as generic `call` nodes (`target: (identifier)` naming `def`/`defp`/`defmacro`/etc.), not a distinguishable dedicated node KIND. Verified via `tree_sitter_elixir`: `defmodule Foo do def pick(a, b) do ... end end` parses to nested `call` nodes, and an ORDINARY function call inside a body (e.g. `Repo.insert()`) has the exact same node kind `call`. This breaks the `function_kinds` node-kind-matching foundation the whole `ComplexitySpec` design relies on (`spec.function_kinds.contains(&node.kind())`) — every call in the file would match, not just definitions. Needs a call-target-filtering mechanism (matching against the `target` field's identifier text, e.g. `def`/`defp`) added as a new generic spec capability before Elixir can be mapped correctly.

## bash

No attribute/annotation grammar construct exists in `tree_sitter_bash`. The one real-world convention (bats-core's `# @test "description"` comment marker) is unstructured free text inside a `comment` node, not a reliable grammar construct — and comments are used for many unrelated purposes, so treating any comment as a potential test marker would be unsafe and overbroad.

## Acceptance

- [x] go, ruby, fortran, swift, elixir each get a grammar-verified `ComplexitySpec` row with the full 6-test suite (match/switch flat, if/elif/else flat, nested loops deepen, boolean run once/mixed twice, test marker exempts, determinism).
- [x] New generic `test_name_prefix`/`test_param_type` mechanism built once (`name_signature_marks_test`) and shared by go, ruby, fortran — no per-language branch.
- [x] Elixir's `function_kinds` matching extended via a new `effective_kind` reclassification mechanism, applied everywhere a node's kind is checked (function detection, nesting, conditionals, arms) — not only function detection, since Elixir's OWN `if`/`unless`/`case`/`cond` turned out to be `call` nodes too (a deeper finding than originally described; documented below). Verified no false positives on ordinary calls (`Repo.insert(a)`, `helper(a)`).
- [x] Swift's `walk_conditional`/`walk_alternative` extended via `walk_marker_conditional` for its wrapper-less, marker-delimited shape. Verified with a real 3-level if/elif/else chain: cognitive_score 4, max_else_if_chain 2, depth stays 1 (flat).
- [x] bash verified genuinely unmappable (see Result below) and left unmapped, reporting not-computed via the existing `COMPLEXITY_NOT_COMPUTED` mechanism in `swissarmyhammer-validators/src/review/probes.rs` — no duplicate not-computed mechanism added.
- [x] No language reports zero when it should report not-computed (`an_unmapped_language_is_not_computed_rather_than_zero` now uses bash as the unmapped exemplar, since go/ruby/fortran/swift/elixir are now mapped).

## Result

**Mapped: go, ruby, fortran, swift, elixir.** Each has a grammar-verified `ComplexitySpec` row and its own 6-test suite in `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs`.

**Deferred: bash.** Independently re-verified the task's own finding by parsing a real bats-core `# @test "..."` sample: the comment is a plain, unstructured `comment` sibling of `function_definition`, indistinguishable by node KIND from an ordinary doc comment or license header. No safe name+signature convention exists for bash either (no de facto standard comparable to Go/Ruby/Fortran's `Test`/`test_` prefix). Bash has no `ComplexitySpec` row and reports not-computed via the pre-existing mechanism.

**New generic mechanisms built (all data-driven via `ComplexitySpec` fields, no per-language branches in the walker):**
- `test_name_prefix` / `test_param_type` / `parameters_field` — name+signature test marking, checked by `name_signature_marks_test`. Used by go (`Test` + `testing.T` param), ruby (`test_` prefix, no param check), fortran (`test_` prefix, no param check).
- `header_child_kinds` / `function_header` — resolves a function's name/parameter-owning node when the function-level container itself carries no fields (fortran's `subroutine`/`function` wrap a `subroutine_statement`/`function_statement` that owns them).
- `condition_field` — parameterizes the previously-hardcoded `"condition"` field name (needed for elixir's `"arguments"`).
- `else_marker_kinds` / `walk_marker_conditional` — swift's positional, marker-token-delimited (`else`) consequence/alternative, field-based condition otherwise.
- `positional_conditional` / `statement_terminator_kinds` / `walk_positional_conditional` — fortran's fully fieldless conditional (condition, consequence, and the `elseif_clause`/`else_clause` chain are all positional siblings).
- `call_target_kinds` / `call_target_test_kinds` / `name_from_call_arguments` / `effective_kind` / `call_target_text` / `call_function_name` — elixir's call-classified special forms.
- `alternative_nested_in_consequence` / `walk_consequence_with_nested_alternative` — elixir's alternative nested as a trailing child inside the SAME consequence container rather than a separate field.
- `child_by_field_or_kind` — elixir's `call` node declares only a `target` FIELD (verified via `node-types.json`); `arguments`/`do_block` are ordinary positional children whose KIND happens to be spelled the same as the concept they hold. Fixed a real bug this uncovered: `child_by_field_name("do_block")`/`("arguments")` silently returned `None` for elixir, which the type system couldn't catch (`Option` swallowed it) — caught only by a test assertion mismatch, root-caused via direct comparison of `field_name_for_named_child` against `child_by_field_name` on the same live node.

**Deeper finding beyond the task's original text:** Elixir's `if`/`unless`/`case`/`cond` are ALL `call` nodes too, not just `def`/`defp`/`defmacro` — the task described the blocker as scoped to `function_kinds`, but the SAME node-kind-collision problem applies to every control-flow construct. Resolved with one generic mechanism (`effective_kind`) applied uniformly everywhere a node's kind is checked, rather than limiting the fix to function detection alone.

**Test-suite adaptation (documented in code, not silent):** Elixir has no imperative loop construct at all (`for` is a functional comprehension, also a `call`; recursion is idiomatic) — `loop_kinds` is intentionally empty, and elixir's "nested loops deepen" test (`elixir_nested_conditionals_deepen_the_score`) uses nested conditionals instead, asserting `max_loop_nesting == 0` explicitly.

**Verification:**
- `cargo fmt --all` clean.
- `cargo clippy -p swissarmyhammer-sem --all-targets -- -D warnings` clean.
- `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean.
- `cargo nextest run -p swissarmyhammer-sem` — 295 passed, 0 failed, 0 skipped.
- `cargo nextest run -E 'rdeps(swissarmyhammer-sem)'` — 3374 passed, 0 failed, 0 skipped (run twice).
- Updated `swissarmyhammer-validators/src/review/probes.rs`'s `complexity_reports_not_computed_for_an_unmapped_language` test to use bash instead of go (go is now mapped).

Scratch grammar-verification harness (`tests/scratch_grammar_dump.rs`) was written, used to verify every node kind/field against the real compiled grammars, and deleted — not part of the final diff.

#bug #review

## Review Findings (2026-08-04 08:17)

- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs:396` — Identical 12-line default field initialization block (test_name_prefix through name_from_call_arguments) repeated verbatim across 7+ ComplexitySpec definitions. This duplication can drift if specs are updated inconsistently, and differs only by parameterized values (test_name_prefix, test_param_type, parameters_field). Extract a parameterized helper function (similar to the existing typescript_family_spec and c_family_spec) that takes test_name_prefix, test_param_type, and parameters_field as arguments and returns a complete ComplexitySpec with common defaults filled in. This reduces duplication from 12+ lines per spec to one function call.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs:864` — Fortran logical operators are only tested in lowercase (`.and.`, `.or.`), but Fortran is a case-insensitive language. The code lists only lowercase operator strings in `logical_operators: &[".and.", ".or."]`, and the test at lines 4693 and 4708 exercises only lowercase source code. Uppercase `.AND.` and `.OR.` (or mixed case) should also be recognized if the tree-sitter grammar normalizes case — but this is not verified by any test. Add one test assertion for Fortran with uppercase logical operators (e.g., `if (a .AND. b .AND. c)`) to verify they are recognized and contribute to the cognitive score and operand count correctly. If the tree-sitter grammar is case-insensitive (which it should be for Fortran), this test should pass; if it's case-sensitive, the code must be corrected.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs:1444` — Fortran test detection uses case-sensitive `starts_with(prefix)` matching, but Fortran is a case-insensitive language. A test subroutine named `TEST_DEEPLY_NESTED` (uppercase) would not be recognized as a test, even though Fortran identifiers are case-insensitive and `TEST_DEEPLY_NESTED`, `test_deeply_nested`, and `Test_Deeply_Nested` all refer to the same subroutine. Make the Fortran test prefix matching case-insensitive. Either normalize the name to lowercase before comparing (`name.to_lowercase().starts_with(prefix)`) or add a case-insensitive comparison. Add at least one test assertion for an uppercase-named test like `TEST_DEEPLY_NESTED` to verify the fix.
