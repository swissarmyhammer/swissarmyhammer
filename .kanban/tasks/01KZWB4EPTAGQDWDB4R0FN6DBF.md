---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzy2t0cwg1e23ymy6ycpz4rf
  text: |
    Pass 1 of 5: the two `swissarmyhammer-kanban` files only. The other four files stay for later passes.

    **What the measurement showed.** In both files the production code is small and the inline `mod tests` block is the bulk:

    | file | production | inline `mod tests` |
    | --- | --- | --- |
    | `scope_commands.rs` | 1318 lines, 53025 raw | 3947 lines, 143236 raw |
    | `dispatch.rs` | 1223 lines, 48388 raw | 3391 lines, 132651 raw |

    Thus the split moves the test block into a `tests/` submodule tree. The production code stays where it is.

    **Where the seams are.** Both files already carry banner comments that name each subject (`// =====` in `scope_commands.rs`, `// -----` in `dispatch.rs`). The banners gave 34 sections and 26 sections. The modules are groups of those sections, so each module has a subject the author already named.

    `scope_commands/tests/`: availability, cross_cutting, dynamic, entity_add, entity_schema, ordering, perspective, scope, templates.
    `dispatch/tests/`: actors_tags, basics, board_columns, collections, comments, dates, perspectives, short_ids, tags, tasks.

    **Shared fixtures go to the parent**, as in `review/fleet/tests.rs`. The parent holds the doc, the mod list, the imports, and only the helpers that more than one child uses: `TestHarness` and `setup` for scope_commands; `setup`, `get_task`, `add_one_task`, `stored_tags` and `add_one_tag` for dispatch. `load_real_views`, `view_by_name` and `make_resolved` have one user each, so they move with that user.

    **One hazard found.** `scope_commands.rs` holds three raw string literals (`stub_yaml`) whose YAML lines start in column 0 or column 4. A blind removal of four spaces of indentation would change the YAML. The split tool keeps the original indentation for lines 4654-4660, 4734-4741 and 4863-4875.

    No defect was found in the moved code.
  timestamp: 2026-08-13T17:30:35.804471+00:00
- actor: claude-code
  id: 01kzy2vbc15j16pfbq5jjgdpfe
  text: |
    **Proof that the move is pure.** Two stages, not a claim.

    Stage 1, before `cargo fmt`: a script read each resulting file, removed the module scaffolding (the `//!` doc, the `mod` list, the `use super::*;` line), put four spaces of indentation back on each surviving line, and put each line at its original line number. The result was compared with the pre-split file from `git HEAD`.

    ```
    PURE MOVE: crates/swissarmyhammer-kanban/src/scope_commands.rs reassembles byte-identical (5267 lines)
    PURE MOVE: crates/swissarmyhammer-kanban/src/dispatch.rs reassembles byte-identical (4615 lines)
    ```

    Stage 2, after `cargo fmt`: the moved lines lost four columns of indentation, so rustfmt re-wrapped some of them. A copy of the stage-1 tree was compared with the formatted tree after all whitespace and all trailing commas were normalised: `23 files compared, 0 differ`. Thus the formatter changed the line breaks only.

    The diff of the two original files is a truncation plus one line each: the only added lines in `dispatch.rs` and `scope_commands.rs` are `mod tests;`. 2 insertions, 7338 deletions.

    **Test count.** 92 tests in `scope_commands`, 153 tests in `dispatch`, before and after.

    - Before: `#[test]` and `#[tokio::test]` attributes in the two files = 92 and 153.
    - After: `cargo nextest list -p swissarmyhammer-kanban` gives 92 under `scope_commands::tests::` and 153 under `dispatch::tests::`.

    **Floor bytes against the 262144 cap.** The largest resulting file is 83787 floor bytes, which is 32 percent of the cap.

    | floor | raw | lines | file |
    | --- | --- | --- | --- |
    | 83787 | 53025 | 1321 | `src/scope_commands.rs` |
    | 77038 | 48388 | 1225 | `src/dispatch.rs` |
    | 41348 | 23104 | 752 | `src/scope_commands/tests/dynamic.rs` |
    | 40327 | 24877 | 625 | `src/dispatch/tests/tasks.rs` |
    | 37976 | 21932 | 652 | `src/scope_commands/tests/ordering.rs` |
    | 37957 | 22199 | 639 | `src/dispatch/tests/collections.rs` |
    | 30598 | 17942 | 498 | `src/scope_commands/tests/entity_add.rs` |
    | 28257 | 15513 | 502 | `src/scope_commands/tests/availability.rs` |
    | 27156 | 16216 | 420 | `src/scope_commands/tests/cross_cutting.rs` |
    | 23044 | 13204 | 370 | `src/dispatch/tests/perspectives.rs` |
    | 23037 | 12229 | 414 | `src/scope_commands/tests/scope.rs` |
    | 22761 | 13361 | 350 | `src/dispatch/tests/basics.rs` |
    | 22222 | 12228 | 377 | `src/scope_commands/tests/perspective.rs` |
    | 20841 | 11969 | 326 | `src/dispatch/tests/short_ids.rs` |
    | 16932 | 9116 | 278 | `src/dispatch/tests/tags.rs` |
    | 15621 | 8641 | 240 | `src/dispatch/tests/actors_tags.rs` |
    | 14655 | 7631 | 242 | `src/scope_commands/tests/entity_schema.rs` |
    | 13938 | 7024 | 237 | `src/dispatch/tests/dates.rs` |
    | 13550 | 7274 | 208 | `src/dispatch/tests/board_columns.rs` |
    | 7603 | 3835 | 94 | `src/dispatch/tests/comments.rs` |
    | 7330 | 3144 | 113 | `src/scope_commands/tests/templates.rs` |
    | 6614 | 3132 | 81 | `src/dispatch/tests.rs` |
    | 5297 | 2321 | 58 | `src/scope_commands/tests.rs` |

    **Verification.**
    - `cargo nextest run -p swissarmyhammer-kanban`: 1600 tests run, 1600 passed, 0 skipped.
    - `cargo nextest run --workspace`: 14181 tests run, 14181 passed, 0 skipped.
    - `cargo fmt --check`: clean.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean.

    **Still open on this card**, for later passes: `swissarmyhammer-sem/src/parser/plugins/code/complexity.rs`, `swissarmyhammer-tools/tests/integration/file_tools_integrations.rs`, `swissarmyhammer-config/src/model.rs`, `swissarmyhammer-entity/src/context.rs`.
  timestamp: 2026-08-13T17:31:19.809066+00:00
- actor: claude-code
  id: 01kzy2vwx403g7kv094t04gthj
  text: |
    ### implement — changed
    - evidence: 23 files — `crates/swissarmyhammer-kanban/src/scope_commands.rs` and `crates/swissarmyhammer-kanban/src/dispatch.rs` truncated to production code plus `mod tests;`; new `src/scope_commands/tests.rs` + 9 modules and `src/dispatch/tests.rs` + 10 modules. Pure move proved by byte-identical reassembly. Test count 92 and 153, before and after. Largest floor 83787 of the 262144 cap. `cargo nextest run --workspace` 14181 passed, `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: four files stay open on this card — `swissarmyhammer-sem/src/parser/plugins/code/complexity.rs`, `swissarmyhammer-tools/tests/integration/file_tools_integrations.rs`, `swissarmyhammer-config/src/model.rs`, `swissarmyhammer-entity/src/context.rs`. The card stays in `doing`.
  timestamp: 2026-08-13T17:31:37.764239+00:00
position_column: doing
position_ordinal: '8280'
title: Six source files are too large for the review engine to read
---
The review engine renders a changed file into one agent prompt. A file whose rendered block passes the 262144-byte per-file cap is dropped from the batch and reported as a "not reviewed, too large" gap. A change that touches such a file gets no review of it.

Measured on 2026-08-12 while splitting `review/fleet/tests.rs` (^q2cncse). Six files pass the cap on the SOURCE RENDER ALONE — that is, before the semantic diff and the probe evidence are added, so they are over the cap for EVERY validator, not just for duplication.

The source render costs `raw bytes + 22 bytes per line` (the `{line:>6} | {sha:8} {mark} | ` columns of `render_numbered_lines`) plus about 1.7 KB of fixed block headers.

| floor bytes | raw | lines | file |
| --- | --- | --- | --- |
| 313824 | 196250 | 5267 | `crates/swissarmyhammer-kanban/src/scope_commands.rs` |
| 302861 | 179721 | 5520 | `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` |
| 284258 | 181028 | 4615 | `crates/swissarmyhammer-kanban/src/dispatch.rs` |
| 276387 | 172673 | 4637 | `crates/swissarmyhammer-tools/tests/integration/file_tools_integrations.rs` |
| 267136 | 166898 | 4479 | `crates/swissarmyhammer-config/src/model.rs` |
| 262659 | 169109 | 4175 | `crates/swissarmyhammer-entity/src/context.rs` |

## What to do

- Split each file into modules named for their subject, the way `review/fleet/tests.rs` and `review/tool_rules/tests.rs` are split. Every resulting file must sit under the cap with headroom.
- Keep each split a pure move: no change to logic, names, or assertions. Measure the test count before and after.

## Done when

- No file in the repository passes the per-file cap on its source render.
- `cargo nextest run --workspace` is green, `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.

#tool-validators