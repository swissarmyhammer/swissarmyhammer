---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzp5b0tpe0skcq319w6qv44c
  text: |-
    Research and measurement, done before the change.

    The card's claim is correct. A test that reads a file with `#[tokio::test(flavor = "multi_thread")] async fn reads_a_row()` fails RED: the read declares `[("fn", "live"), ("fn", "reads_a_row")]`, so the decorated function is not read as test code.

    The card's counts came from a tree of 1183 tracked `.rs` files. This tree holds 1189. Measured with a release `sah` over every tracked `.rs` file:

    - before the change: 414 findings
    - after the change: 406 findings

    The delta is 8, which is the delta the card measured. The 8 removed findings are the 7 `#[tokio::test(...)]` functions of `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs` and one in `crates/swissarmyhammer-tools/tests/integration/review_e2e.rs`. No finding is added: a line-for-line comparison of the two reports shows 8 removals and 0 additions.

    So the number the rule body carries is 406, not the 408 the card names. The tree moved under the card.
  timestamp: 2026-08-10T15:40:54.998778+00:00
- actor: claude-code
  id: 01kzp5mfm8md049pmmse94yg0w
  text: |-
    ### implement — changed

    - files:
      - `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs`
      - `builtin/validators/duplication/rules/duplication-parsed.md`

    The fix: `attribute_names` now reads the stripped text, and its last path
    segment, through a new `names_marker` helper. The helper answers true when the
    text IS the marker, or when the marker is followed by `(`. The argument list
    must open right after the marker, so `#[test_case(1)]` does not name `test`.

    Two tests, both through the real tree-sitter parse:

    - `a_rust_test_attribute_that_carries_arguments_contributes_no_definition` —
      RED before the fix (the read declared `reads_a_row`), GREEN after.
    - `a_rust_cfg_test_function_contributes_no_definition` — the guard the card
      asks for. It is not empty: with the equality arm of `names_marker` taken out
      it fails, and it passes with the arm back.

    Measured counts, release binary, every tracked `.rs` file:

    - before 414, after 406, 8 removed and 0 added.
    - with the Rust test markers taken out of the table: 3216, so the structural
      exclusion is worth 2810 findings, 87.4 percent of the raw total. The rule
      body carried 3490 and 88.1 percent from the 1183-file tree; both are
      re-measured.

    The rule body now states 406 over 1189 files and names the eight — seven in
    `src/mcp/tools/review/tests.rs`, one in `tests/integration/review_e2e.rs`. The
    gate table, the bounds section and the window comparison keep their 1183-file
    numbers: each is a record of a measurement that chose a gate or measured a past
    change, and none can be re-run (the gates are constants and the window engine
    is deleted).

    Commands: `cargo fmt --all --check` clean, `cargo clippy --workspace
    --all-targets -- -D warnings` clean, `cargo nextest run --workspace` 14026
    tests, 14022 passed, 4 failed.

    The 4 failures are the known ones of ^bh5ncd0 and not mine. Proved rather than
    assumed: with the two changed files put back to their HEAD content, the same
    three `review_e2e` tests fail with the same text at the same lines
    (`review_e2e.rs:161:5` "duplication via sha", `:282:5` "a confirmed finding
    must land on the task"), and `review_e2e_file_glob_confirms_scoped_defects`
    passes in both runs. The claim those tests miss, `CLAIM_DUP`, comes from a
    scripted agent response in `review_fixture.rs`, and it is planted on production
    code (`pub fn sum_amounts`), so no test-marker read can reach it.
  timestamp: 2026-08-10T15:46:05.064276+00:00
- actor: claude-code
  id: 01kzp5zvagjg3kfpzpj4ten6sk
  text: |-
    ### review — findings
    - evidence: 2 findings, 2 confirmed, 0 refuted, 8 attempted — review sha HEAD~1..HEAD (6cfa33fac); crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs:318, crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs:714
    - next: Make `definition_spec` and `test_spec` data tables. Keep the fallback specs. The card stays in review.
  timestamp: 2026-08-10T15:52:17.488162+00:00
- actor: claude-code
  id: 01kzp60mzkgm436ansqh8h18e3
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 2 files; `attribute_names` now goes through a new `names_marker` helper that accepts the bare marker or the marker followed by `(`; 2 regression tests, both RED before and GREEN after
    - measured: the card's 416 baseline was stale. The tree holds 1189 tracked `.rs` files, not 1183. The real baseline is 414 and the result is 406. The delta of 8 is the same. Seven of the eight are in `src/mcp/tools/review/tests.rs` and one is in `tests/integration/review_e2e.rs`; the card said all eight were in one file.
    - test: `cargo nextest run --workspace` 14026 tests, 14022 passed, 0 skipped; the 4 failures are the known ^bh5ncd0 set, and the text is identical with the two changed files put back to their HEAD content
    - commit: 6cfa33fac
    - review: findings — crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs:318, crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs:714
  timestamp: 2026-08-10T15:52:43.763756+00:00
- actor: claude-code
  id: 01kzp76adbb9q67r5w0034wtx7
  text: |
    Both findings of the 2026-08-10 10:47 section are done. The two lookups are now
    data.

    The file holds ONE roster, and not two tables. A `LanguageSpec` row carries the
    language id, its `DefinitionSpec` and its `TestSpec`, and `LANGUAGE_SPECS` holds
    16 rows — one for each language. A new language is one row, so the two halves
    cannot drift apart. This is the shape `public_surface.rs` in the same directory
    already uses: a `SurfaceSpec` roster with a `language` field and a
    `spec_for_language` lookup.

    `definition_spec` and `test_spec` are now one line each. Each reads the roster
    through `language_spec` and keeps its fallback:

        fn definition_spec(language: &str) -> &'static DefinitionSpec {
            language_spec(language).map_or(&NO_DEFINITION_SPEC, |spec| spec.definitions)
        }

    Bash and Fortran hold `NO_TEST_SPEC` as a written row, so the reason they get no
    test marker is data and no longer a comment on a fallback arm.

    The rest of the file was read for the same cause. Two `match` statements stay,
    and neither is a table: `match normalization` is over the `Normalization` enum,
    which the compiler checks for exhaustiveness (the `data-driven` rule carves this
    out word for word), and `match annotated_sibling(marker)` is over an `Option`.

    `is_identifier_kind` held the same cause in a different spelling — four `kind ==`
    comparisons and three `kind.ends_with` comparisons joined by `||`. It is now
    `IDENTIFIER_KINDS` plus `IDENTIFIER_KIND_ENDINGS`, read the way
    `is_definition_kind` two functions below already reads `DEFINITION_KINDS` plus
    `DEFINITION_KIND_ENDINGS`. A finding names one example of a cause, so the cause
    comes out of the whole file.

    New test: `each_language_of_the_roster_is_named_one_time`. It holds every row id
    of the roster to one entry of a set. RED was proved, not assumed: with a second
    `"rust"` row put in, the test fails with "`rust` holds more than one row, and the
    lookup reads only the first"; with the row taken out, it passes. The guard
    matters because the lookup reads the first row it finds, so a second row for one
    language is silent.
  timestamp: 2026-08-10T16:13:18.123048+00:00
- actor: claude-code
  id: 01kzp76nj8g3p56y8cemxkbnh0
  text: |
    ### implement — changed
    - evidence: 1 file — `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs`. Both findings of the 2026-08-10 10:47 section are now `- [x]`. `definition_spec` and `test_spec` read the new `LANGUAGE_SPECS` roster (16 rows) through `language_spec`, and each keeps its fallback spec. `is_identifier_kind` holds the same cause and is now `IDENTIFIER_KINDS` plus `IDENTIFIER_KIND_ENDINGS`. One new test, `each_language_of_the_roster_is_named_one_time`, proved RED with a second `"rust"` row and GREEN without it.
    - counts: `cargo nextest run --workspace` 14027 tests, 14023 passed, 0 skipped, 4 failed. The 4 are the known ^bh5ncd0 set — three `review_e2e` tests and `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport`. `the_swift_package_root_restores_the_directory_before_it_removes_it` passed. The two regression tests of the last pass, `a_rust_test_attribute_that_carries_arguments_contributes_no_definition` and `a_rust_cfg_test_function_contributes_no_definition`, both pass.
    - duplication: 406, measured with a release `sah` over all 1189 tracked `.rs` files in ONE process. The report is identical line for line before and after the `is_identifier_kind` change, so the count did not move.
    - commands: `cargo fmt --all -- --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo nextest run --workspace`.
    - next: `/review`. The card stays in `doing`.
  timestamp: 2026-08-10T16:13:29.544373+00:00
position_column: doing
position_ordinal: '8480'
title: duplication test exclusion misses a test attribute that carries arguments
---
Found while measuring `^80nbway`.

`attribute_names` in `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs` compares the stripped attribute text against the marker for equality only. An attribute that carries arguments therefore names nothing:

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]

strips to `tokio::test(flavor = "multi_thread", worker_threads = 2)`, whose last path segment is `test(flavor = ...)`, which is not `test`. The function is not read as test code, so `duplication-parsed` reports it.

The plain `#[tokio::test]` form works. `#[cfg(test)]` works because the table lists the whole text `cfg(test)`.

## Measured

Over the 1183 tracked `.rs` files of this workspace the rule reports **416** findings. With the argument form read, it reports **408**. So the gap costs 8 wrong findings, all of them `#[tokio::test(...)]` functions in `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`.

The workspace holds 107 `tokio::test(` sites.

## The fix, already measured

Read an attribute as naming a marker when its text is the marker OR starts with the marker followed by `(`:

    fn names(text: &str, marker: &str) -> bool {
        text == marker || text.strip_prefix(marker).is_some_and(|rest| rest.starts_with('('))
    }

Apply it to the stripped text and to its last path segment, as `attribute_names` already does for equality. Measured: 416 -> 408.

## Acceptance

- `#[tokio::test(flavor = "multi_thread")]` marks the function it decorates as test code
- `#[cfg(test)]` keeps working
- The rule body's measured count is updated to the new number

#tool-validators

## Review Findings (2026-08-10 10:47)

- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs:318` — The `definition_spec` function uses a match statement over a known set of language strings, with each arm returning a different static reference. This is a lookup table written as control flow and should be expressed as data to avoid drift when languages are added. Replace the match statement with a table (e.g., `&[(&str, &DefinitionSpec)]` or a lazy_static HashMap) that maps language IDs to their corresponding DefinitionSpec. Preserve the fallback to NO_DEFINITION_SPEC as the default for unrecognized languages.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs:714` — The `test_spec` function uses a match statement over a known set of language strings, with each arm returning a different static reference. This is a lookup table written as control flow and should be expressed as data to avoid drift when languages are added. Replace the match statement with a table (e.g., `&[(&str, &TestSpec)]` or a lazy_static HashMap) that maps language IDs to their corresponding TestSpec. Preserve the fallback to NO_TEST_SPEC as the default for unrecognized languages.
