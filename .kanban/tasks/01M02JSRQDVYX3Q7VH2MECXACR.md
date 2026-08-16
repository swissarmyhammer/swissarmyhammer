---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m03zp6pvvz18w093yvsb5ybz
  text: |-
    Picked up. Research done before any edit.

    ## Consumer search — whole workspace

    `grep -rn` over `*.rs`/`*.toml`/`*.md`/`*.yaml`/`*.yml`/`*.json` for `cognitive_complexity|COGNITIVE_COMPLEXITY_THRESHOLD|NESTING_DEPTH_THRESHOLD|FunctionComplexity|FileComplexity|ComplexitySpec|complexity::|test_census|TestCensus|TestDefect`, excluding the `complexity/` tree itself and `target/`. 107 hits. Only four are source:

    - `crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs` — uses `test_census`, `TestCensus`. This is the live consumer the card says must keep working. It does NOT touch the scorer.
    - `crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs` — `mod complexity;`, the scorer re-export, the census re-export, and three module-doc paragraphs.
    - `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs` doc line — a prose reference to the `complexity` copy of `is_test_definition`.
    - `builtin/validators/code-hygiene/VALIDATOR.md` — `clippy::cognitive_complexity` in the rejected-tools table. That is the CLIPPY LINT, not this scorer. KEPT.

    Everything else is `.kanban/tasks/*.md` history. **No consumer of the scorer half exists anywhere.** Nothing blocks the deletion.

    ## What `test_census.rs` actually imports

    Twelve items, not nine. The `use super::{...}` line names nine (`child_by_field_or_kind`, `for_each_function`, `function_header`, `function_name`, `is_test_definition`, `node_text`, `spec_for_language`, `ComplexitySpec`, `MAX_TRAVERSAL_DEPTH`); the body calls three more by path (`super::definition_attributes`, `super::attribute_marker_name`, `super::defining_call`).

    Not one of the twelve is complexity scoring. Every one answers "where is a function definition, what is it named, and is it a test?" — read off a per-grammar data row. That is a grammar layer that happened to live inside the scorer file.

    ## The split

    Definition layer (moves, keeps all 15 grammar rows): the spec struct and its rows, `spec_for_language`, `ALL_SPECS`, `for_each_function`, `effective_kind`, `call_target_text`, `defining_call`, `call_function_name`, `STRING_QUOTES`, `function_name`, `function_header`, `resolve_declarator_name`, `node_text`, `child_by_field_or_kind`, `is_test_definition`, `definition_attributes`, `collect_attributes`, `attribute_marks_test`, `attribute_marker_name`, `name_signature_marks_test`, `MAX_TRAVERSAL_DEPTH`.

    Scoring (deleted): `COGNITIVE_COMPLEXITY_THRESHOLD`, `NESTING_DEPTH_THRESHOLD`, `FunctionComplexity`, `FileComplexity`, `cognitive_complexity`, `collect_functions`, `Tally`, `score_function`, and the twelve walkers (`walk`, `walk_children`, `walk_conditional`, `walk_marker_conditional`, `walk_positional_conditional`, `walk_consequence_with_nested_alternative`, `walk_alternative`, `carries_label`, `logical_operator`, `is_boolean_root`, `walk_boolean`, `boolean_chain`), plus `complexity/tests.rs` and its 13 language modules.

    Seventeen of the spec struct's fields are read only by the walkers (`nesting_kinds`, `conditional_kinds`, `consequence_field`, `elif_kinds`, `else_wrapper_kinds`, `loop_kinds`, `arm_kinds`, `nest_only_kinds`, `logical_kinds`, `logical_operators`, `labelled_jump_kinds`, `label_kinds`, `condition_field`, `else_marker_kinds`, `positional_conditional`, `statement_terminator_kinds`, `alternative_nested_in_consequence`). They go with the walkers — `dead_code` fires on a never-read field, so leaving them would fail `clippy -D warnings` anyway.

    ## Watch-for from ^4dyewvd, honoured

    `node_text` and `spec_for_language` MOVE unchanged, each keeping its own contract and its own static table. Neither is unified with a sibling. The `plugins/code` module doc that records that decision is updated to name the new module rather than deleted.
  timestamp: 2026-08-16T00:31:32.059112+00:00
- actor: claude-code
  id: 01m040v2afphw2rkykf6jdtqff
  text: |-
    Implementation landed. The split the research comment described, carried out.

    ## What was deleted

    - `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` (2,256 lines) and the whole `complexity/` tree: `tests.rs` plus its 13 language modules (3,268 lines). 15 files, 5,524 lines.
    - The scorer's public surface, and its `pub use` in `plugins/code/mod.rs`: `cognitive_complexity`, `FileComplexity`, `FunctionComplexity`, `COGNITIVE_COMPLEXITY_THRESHOLD`, `NESTING_DEPTH_THRESHOLD`.
    - The scorer's internals: `collect_functions`, `Tally`, `score_function`, and the twelve walkers.
    - The 17 spec fields only the walkers read (`nesting_kinds` … `alternative_nested_in_consequence`).
    - Elixir's control-flow entries in `call_target_kinds` — `if`, `unless`, `case`, `cond`. They reclassified a `call` node so the walker's nesting checks could see it. The definition path reads that list only against `function_kinds`, which names none of the four, so removing them changes no answer.
    - `use super::parse_code;` in the deleted file — the scorer was its only user there.
    - The stray `/// Cognitive complexity computed from the parse …` paragraph in `mod.rs`, which sat above the `commented_code` re-export rather than above its own. `commented_code` now carries its own doc line, so the deletion leaves no undocumented re-export behind.

    ## Where the grammar layer went

    New `crates/swissarmyhammer-sem/src/parser/plugins/code/definitions.rs`. `ComplexitySpec` becomes `DefinitionSpec`, `ALL_SPECS` becomes `DEFINITION_SPECS`, and the module holds only the three questions the census asks: where a definition is, what it is called, whether it is marked as a test.

    `complexity/test_census.rs` moves up to `code/test_census.rs` (`git mv`, so the rename is recorded). Its twelve imports now come from `super::definitions`; the local `complexity` binding is renamed `definitions`. No assertion in it changed.

    Visibility is precise rather than blanket: twelve items are `pub(super)` because `test_census` names them; `DEFINITION_SPECS`, the 15 rows, `SPEC_DEFAULTS`, `effective_kind`, `call_target_text`, `call_function_name`, `STRING_QUOTES`, `resolve_declarator_name`, `collect_attributes`, `attribute_marks_test` and `name_signature_marks_test` stay private. Of the struct's 16 fields only `name_field` and `parameters_field` are `pub(super)` — the two `body_statements` reads.

    ## Borderline items KEPT, and why

    1. **All 15 grammar rows**, including the seven (C, C++, C#, PHP, Fortran, Swift, Elixir) with no census vocabulary. The card protects exactly this: "a language-spec lookup … is not complexity scoring". They are the verified grammar mapping a census row is added ON TOP of. The Elixir row is load-bearing for two live tests — `test_census.rs::a_language_with_no_census_mapping_is_not_measured` and `tree_sitter_probes.rs::a_language_with_no_census_mapping_reports_one_not_computed_row` — which assert "recognized as a test definition, yet not measured". Delete the row and both still pass, for the wrong reason. The new module doc names the seven and states why they stay.
    2. **`header_child_kinds`** (Fortran only), **`test_name_case_insensitive`** (Fortran only), and **`resolve_declarator_name`'s declarator chain** (C/C++ only). Each is read by `function_header`/`name_signature_marks_test`/`function_name`, all live.
    3. **`MAX_TRAVERSAL_DEPTH`** — read by `for_each_function` and by the census's own `for_each_descendant`. Its doc no longer cites the deleted walkers.
    4. **`builtin/validators/code-hygiene/VALIDATOR.md:449`, `clippy::cognitive_complexity` — rejected.** That is the CLIPPY LINT verdict from ^d3dfhnxg, not this scorer. Untouched.
    5. **The `^4dyewvd` record in `plugins/code/mod.rs`** — the four `node_text` contracts and the four `spec_for_language` tables. Renamed to `definitions`/`DefinitionSpec`/`DEFINITION_SPECS` rather than deleted. The dated line still names `complexity.rs` and now adds "whose surviving half is now `definitions.rs`", so the measurement stays traceable to the file it was taken on. `node_text` and `spec_for_language` MOVED unchanged and were not unified with any sibling.
    6. **Every `tree-sitter-*` dependency in `Cargo.toml`** — `languages.rs` routes 16 languages through them. None was scorer-only.

    ## Dangling-reference sweep

    `grep -rn` over `*.rs`/`*.toml`/`*.yaml`/`*.yml`/`*.json`/`*.md` (excluding `target/` and `.kanban/`) for `cognitive_complexity|COGNITIVE_COMPLEXITY_THRESHOLD|NESTING_DEPTH_THRESHOLD|FunctionComplexity|FileComplexity|ComplexitySpec|ALL_SPECS|EXTENDED_SPEC_DEFAULTS|complexity\.rs|complexity::|code/complexity` returns exactly the two intentional hits above. No build script, `include_dir!` or `include_str!` in either crate names the deleted tree. Doc-prose references in `duplication.rs` (two), `entity_extractor.rs` and `languages.rs` were updated to `definitions`.

    ## Verification

    - `cargo fmt --all --check` clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings, 0 errors.
    - `cargo nextest run --workspace` — **14,105 run, 14,105 passed, 0 failed, 0 skipped.**
    - `cargo nextest run -E 'rdeps(swissarmyhammer-sem)'` — 5,256 run, 5,256 passed, 0 failed, 0 skipped.
    - The live consumer specifically: `swissarmyhammer-validators` `a_language_with_no_census_mapping_reports_one_not_computed_row`, `a_test_that_asserts_nothing_is_the_only_row_the_census_reports`, `test_test_integrity_declares_the_assertion_census_probe_its_rule_reads` — 3 passed.
    - Clean-metadata build: `cargo clean -p swissarmyhammer-sem` then rebuild — clean.
    - `cargo doc -p swissarmyhammer-sem --no-deps --document-private-items` — 16 warnings, the same 16 that stood before this change (13 "links to private item", which is the shape `commented_code`/`duplication`/the old `complexity` re-exports already had, and 2 unrelated `toml_plugin` ones). The four warnings the first draft added — one `redundant_explicit_links` and three ambiguous `super::test_census` links — are fixed with the shorthand and `mod@`.

    ### implement — changed
    - evidence: 18 files. Deleted `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` + `complexity/tests.rs` + 13 `complexity/tests/*.rs`. Added `crates/swissarmyhammer-sem/src/parser/plugins/code/definitions.rs`. Renamed `complexity/test_census.rs` -> `test_census.rs`. Edited `code/mod.rs`, `code/duplication.rs`, `code/entity_extractor.rs`, `code/languages.rs`. `cargo nextest run --workspace` 14105 passed 0 failed 0 skipped; `cargo clippy --workspace --all-targets -- -D warnings` 0 warnings; `cargo fmt --all --check` clean.
    - next: `/review`. Not committed; the card stays in `doing`.
  timestamp: 2026-08-16T00:51:39.983907+00:00
- actor: claude-code
  id: 01m041n5zpbw2vtqchsd5sr41d
  text: |
    ### CORRECTION — the stash is popped. Ignore the stash warning above.

    The comment above says the work of this card stands in `stash@{0}`. That was true when it was written. It is NOT true now.

    The stash is popped and dropped. Every change of this card stands in the working tree:

    - `crates/swissarmyhammer-sem/src/parser/plugins/code/definitions.rs` — present, 44204 bytes.
    - `crates/swissarmyhammer-sem/src/parser/plugins/code/test_census.rs` — present, 29260 bytes.
    - `complexity.rs` and the whole `complexity/` tree — gone, 15 deletions recorded by git.
    - `duplication.rs`, `entity_extractor.rs`, `languages.rs`, `mod.rs` — modified.

    `git stash list` is empty. Do NOT look for a stash, and do NOT re-implement this card.

    ### Where this card stands

    - implement: DONE. Measured green before the interruption — `cargo nextest run --workspace` 14105 passed, `cargo clippy --workspace --all-targets -- -D warnings` 0 warnings, `cargo fmt --all --check` clean, clean-metadata rebuild clean.
    - test: INCOMPLETE. The step ran the dangling-reference sweep, which answered clean with only the two expected hits, and it confirmed the test-count accounting closes exactly (406 - 135 + 19 = 290; the 19 are the census tests that MOVED, the 116 others are the deleted scorer's own tests). It was re-running the full suite when the session reached its usage limit and the agent was stopped.
    - commit: NOT DONE.
    - review: NOT DONE.

    The card stays in `doing`. The next run repeats `/test` to get a fresh green on the restored tree, then `/commit`, then `/review`.
  timestamp: 2026-08-16T01:05:55.702353+00:00
- actor: claude-code
  id: 01m05720f01wts9wcn022kf6w8
  text: |
    ### test — green

    - evidence:
      - `cargo fmt --check` — exit 0, no diff.
      - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
      - `cargo nextest run --workspace` — 14105 tests run: 14105 passed, 0 failed, 0 skipped.
      - `cargo nextest run -p swissarmyhammer-sem` — 290 tests run: 290 passed, 0 failed, 0 skipped (matches the 406 - 135 + 19 = 290 accounting).
      - `cargo doc -p swissarmyhammer-sem --no-deps` — 8 `rustdoc::private_intra_doc_links` warnings, all confirmed pre-existing (not caused by this change): 4 point at `[`COMMENT_SPECS`]` in `commented_code.rs`, a file untouched by this diff, present at HEAD unchanged; 4 point at `[`duplication`]` in `mod.rs` line 83, confirmed present verbatim at HEAD (`git show HEAD:...mod.rs | grep '`duplication`'`) before this diff touched that file. Zero new warnings.
      - Dangling-reference sweep (read-only, no stash): `grep -rn -E "cognitive_complexity|FileComplexity|FunctionComplexity|COGNITIVE_COMPLEXITY_THRESHOLD|NESTING_DEPTH_THRESHOLD|ComplexitySpec|ALL_SPECS" crates/ builtin/` returns exactly one hit — `builtin/validators/code-hygiene/VALIDATOR.md:449` (`clippy::cognitive_complexity`, the clippy lint, not this code). The second expected hit is history prose in `crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs` line 53, which names the file `complexity.rs` in prose (not one of the deleted symbol names, so it does not match the symbol grep, but is the same deliberate history note the earlier run flagged). Confirmed clean.
    - No tree mutation: only read-only git commands used (`git status`, `git diff`, `git log`, `git show`).
    - Task left in `doing`, not committed, per instructions.
  timestamp: 2026-08-16T11:59:33.344549+00:00
position_column: doing
position_ordinal: '8280'
title: Delete the cognitive-complexity scorer from swissarmyhammer-sem
---
Delete the complexity scorer. Not "retire or split" — delete it. Complexity is not a measured concern any more; the only size gate is function length.

The user's instruction, verbatim: *"retire it — we needed to get rid of complexity scoring — like rid, we are just doing function length, I thought I was clear."*

## What survived and why it must not

`^z2r1psf` removed the five `complexity-<lang>` tool rules, the `cognitive-complexity` prompt rule and the probe WIRING, but kept the scorer in `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` and its `complexity/` tree. The reason given was that `test_census.rs` imports nine items from it and has its own live consumer in `tree_sitter_probes.rs`.

That is a dependency to break, not a reason to keep dead code. The scorer now has no consumer of its own — it is kept alive solely by a neighbour that reached into it.

## What to do

1. Establish exactly what `test_census.rs` uses from `complexity.rs` — the nine imported items, and whether each is genuinely about complexity or is a general tree-sitter helper that merely lives there.
2. Move what `test_census` legitimately needs to where it belongs. A language-spec lookup or a node-text helper is not complexity scoring and should not be deleted with it; the scoring itself is.
3. Delete the scorer and its `complexity/` tree.
4. Sweep for anything else reaching into it, inside and outside the crate.

## Watch for

The measurement recorded on `^4dyewvd` applies here: the four `node_text` copies in this tree have four different contracts, and `spec_for_language` reads four unrelated static tables. If `test_census` needs one of those, do not unify it with its siblings while moving it — that question was settled against a shared module, and the reasoning is written into `plugins/code/mod.rs`.

## Done when

- No complexity scoring code remains in `swissarmyhammer-sem`.
- `test_census` compiles and its live consumer still works.
- `cargo nextest run --workspace` green; fmt and clippy clean.

#tool-validators