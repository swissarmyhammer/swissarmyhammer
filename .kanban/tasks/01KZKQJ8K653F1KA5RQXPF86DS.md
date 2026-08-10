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