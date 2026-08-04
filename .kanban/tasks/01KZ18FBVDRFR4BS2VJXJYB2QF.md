---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz5hzwb86prkrf21k6kvcaxp
  text: |-
    ### implement — changed

    - Languages mapped, grammar-verified, full 6-test suite (74 new/updated tests total in `complexity.rs`, all green): **typescript, tsx, javascript, python, java, c, cpp, csharp, php**.
    - Verification method: for each language, wrote temporary `crates/swissarmyhammer-sem/tests/grammar_dump*.rs` scratch harnesses that parsed real samples with the actual `tree_sitter_typescript`/`tree_sitter_javascript`/`tree_sitter_python`/`tree_sitter_java`/`tree_sitter_c`/`tree_sitter_cpp`/`tree_sitter_c_sharp`/`tree_sitter_php` crates and printed `Node::to_sexp()` plus exact byte-span text for attribute/annotation/decorator nodes. Deleted all scratch files (`git status` confirms none remain) before finishing. Ran ~8 rounds covering: control flow (if/elif/switch/loops/do-while/boolean operators), attribute/annotation/decorator placement (sibling vs. embedded vs. container-wrapped), and repeated vs. single `alternative` field shape for else-if chains.
    - Found `csharp`'s `LanguageConfig.id` is `"csharp"`, not `"c_sharp"` as the card's language list said — used the real id.
    - Core design change (generic, not per-language branching): replaced Rust's `else_kinds`-scan-all-children mechanism with a `consequence_field`/`elif_kinds`/`else_wrapper_kinds`-driven `walk_conditional`/`walk_alternative` pair, because the 9 languages split into three distinct `if`/`else-if`/`else` AST shapes verified via the dumps:
      - single recursive `alternative` field wrapped in a transparent `else_clause` (Rust, C, C++, JavaScript, TypeScript)
      - single bare `alternative` with no wrapper — Java's/C#'s `alternative` holds the next `if_statement` or terminal `block` directly
      - repeated `alternative` field flattened onto the original conditional (Python's `elif_clause`, PHP's `else_if_clause`, confirmed with 3-way chains — neither nests)
      All existing Rust tests pass unchanged with the new fields (`consequence_field: "consequence"`, `elif_kinds: []`, `else_wrapper_kinds: ["else_clause"]`).
    - `is_test_definition` generalized the same way: still scans preceding siblings (Rust's `#[attr]`, Python's decorator-via-`decorated_definition`-wrapper, TypeScript's decorator-as-`class_body`-sibling — confirmed TS decorator is a true sibling, unlike JS), and now ALSO scans the definition's own children directly (JavaScript's `decorator:` field on `method_definition` itself) and container-wrapped children (Java's `modifiers`, C#'s `attribute_list`, PHP's two-level `attribute_list > attribute_group`, C/C++'s `attribute_declaration`). `attribute_marks_test` generalized to strip `#`/`@`/`[`/`]` and split on `.`/`::`/`\`, case-insensitively, covering Rust's `#[test]`, Python's `@pytest.mark.test`, Java's `@Test`, C#'s `[Test]`, PHP's real PHPUnit `#[Test]`, JS/TS's `@Test`, and C/C++'s synthetic `[[test]]` (C/C++ have no idiomatic test-attribute convention, but the C++11/C23 `[[...]]` attribute mechanism is real and grammar-verified; used a synthetic marker to demonstrate the wiring).
    - `function_name` generalized via a new `resolve_declarator_name` that unwraps a nested `declarator` field chain, needed for C/C++ where the name sits several `declarator` levels deep (`function_definition.declarator` → `function_declarator.declarator` → identifier, one more `pointer_declarator` level for a pointer return type). Every other language's name field already points at a leaf, so the same generic function is a no-op recursion base case for them.
    - Fixed a knock-on test break: `swissarmyhammer-validators`' `complexity_reports_not_computed_for_an_unmapped_language` used `src/app.py` as its "unmapped language" example; since Python is now mapped, moved it to `src/app.go` (Go remains genuinely unmapped).
    - Deferred: go, ruby, fortran, swift, elixir, bash — filed as ^xsqz5hd with the specific blocker recorded per language (no attribute mechanism at all for go/ruby/fortran; Swift's if-else has no `alternative`-field wrapper shape my walker design handles; Elixir's functions are generic `call` nodes indistinguishable by kind from ordinary calls).
    - Verification: `cargo nextest run -p swissarmyhammer-sem -p swissarmyhammer-validators` → 621/621 green. `cargo nextest run -E 'rdeps(swissarmyhammer-sem)'` → 3328/3328 green. `cargo fmt --all` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.

    next: /review
  timestamp: 2026-08-04T04:54:56.104564+00:00
position_column: doing
position_ordinal: '8380'
title: complexity scorer covers only Rust — map the remaining source_code languages
---
The `complexity` probe computes Sonar cognitive complexity from the tree-sitter parse (see `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs`). It had one `ComplexitySpec` row: Rust.

Every other language the `complexity` validator matches (`@file_groups/source_code`) reported **not computed**. That is deliberate and safe — a missing mapping never reads as a score of zero — but it meant the agent still judged those languages by eye, which is the drift ^k5wsxh0 removed for Rust.

## Work

Add a `ComplexitySpec` row per language. `swissarmyhammer-sem` already carries the grammars and the `LanguageConfig` for: typescript, tsx, javascript, python, go, java, c, cpp, ruby, c_sharp, php, fortran, swift, elixir, bash.

For each row, verify the node kinds against the real grammar (parse a sample and read the s-expression) rather than guessing. The existing Rust row was built that way.

Each language needs the same test set the Rust row has:

- a `match`/`switch` scores once and its arms open no nesting level
- an if/else-if/else chain is flat
- nested loops deepen the score
- a boolean run scores once, a mixed run twice
- the test marker at the definition exempts the function
- repeated scoring never drifts

## Result

9 of 15 languages done, fully grammar-verified with the complete 6-test suite: **typescript, tsx, javascript, python, java, c, cpp, csharp (id `csharp`, not `c_sharp`), php**.

6 languages deferred to ^xsqz5hd, each on a specific, named blocker (not missing effort) — see that card for detail: **go, ruby, fortran** (no attribute/annotation grammar construct for test-marking at all), **swift** (real `@Test` attribute exists, but its if/else-if/else shape has no `alternative`-field wrapper the shared walker relies on), **elixir** (functions are generic `call` nodes indistinguishable by kind from ordinary calls, breaking the `function_kinds` matching foundation).

Along the way, the shared walker was generalized from Rust's `else_kinds`-scan to a `consequence_field`/`elif_kinds`/`else_wrapper_kinds`-driven `walk_conditional`/`walk_alternative` pair, since the 9 new languages use three distinct `if`/`else-if`/`else` AST shapes (Rust/C/C++/JS/TS's single-`alternative`-with-`else_clause`-wrapper; Java's/C#'s single bare `alternative`; Python's/PHP's repeated flat `alternative`). `is_test_definition` was generalized the same way, to also check a definition's own children (JS's `decorator` field) and container-wrapped children (Java's `modifiers`, C#'s `attribute_list`, PHP's `attribute_list`/`attribute_group`, C/C++'s `attribute_declaration`), not just preceding siblings. All Rust tests still pass unchanged.

## Acceptance

- [x] Every extension in `builtin/file_groups/source_code.yaml` that `swissarmyhammer-sem` can parse has a spec row — **partial**: 9/15 languages done and verified; 6 deferred to ^xsqz5hd with specific blockers recorded there.
- [x] A language with no grammar still reports not-computed, never zero — verified (updated the validators-side `complexity_reports_not_computed_for_an_unmapped_language` test, which had used Python as its "unmapped" example and needed to move to Go since Python is now mapped).
- [x] The per-language node kinds are verified against the grammar, not assumed — see the implement comment for the verification method (temporary `tests/grammar_dump*.rs` scratch harnesses, parsed with the real `tree_sitter_*` crates, deleted after use).

#bug #review