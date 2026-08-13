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
- actor: claude-code
  id: 01kzy7mcw2svasheq7d07cyen9
  text: |
    Pass 2 of 2: the four remaining files. Method as pass 1 — find the author's own seams, move a test block into a `tests/` tree, keep only the fixtures more than one child uses in the parent.

    **Where the seams are.**

    | file | shape | seams the author already wrote |
    | --- | --- | --- |
    | `complexity.rs` | production 2236 lines, inline `mod tests` 3283 lines | `// ----` banners name one language for each section |
    | `file_tools_integrations.rs` | integration test file, no production code | `// ====` banners name one tool or subject for each section |
    | `model.rs` | production 1632 lines, two inline test modules | few banners; the test names group by subject |
    | `context.rs` | production 1810 lines, inline `mod tests` 2362 lines | four banners; the test names group by subject |

    **How each file was cut.**

    `complexity.rs` — the test block moves to `complexity/tests.rs` plus 13 modules, one for each `ComplexitySpec` row: `rust`, `typescript_family` (TypeScript, TSX and JavaScript, the three rows that share `typescript_family_spec`), `python`, `java`, `c_family` (C and C++, which share `c_family_spec`), `csharp`, `php`, `depth`, `go`, `ruby`, `fortran`, `swift`, `elixir`. The parent keeps `DETERMINISM_RUNS`, `only_function`, `only_function_for`, `method_in_class` (Java and C#) and the two raw-string Rust fixtures `COLLECT_LINE_TAGS` and `EDIT_LINE_MARKERS` (Rust and depth). `php_source` has one user, so it moves to `php`; `deeply_nested_if_source` moves to `depth`.

    `file_tools_integrations.rs` — no production code, so the parent keeps the doc, the imports and the shared harness, and 10 modules take the tests: `read`, `glob`, `grep`, `write`, `edit`, `composition`, `security`, `performance`, `concurrency`, `properties`. The parent keeps `create_test_context`, `create_test_registry`, `extract_response_text`, `read_content`, `create_test_file`, `create_test_dir_with_git`, the five argument builders, `run_concurrent_test`, `MemoryProfiler` and the four `verify_tool_*` checks. The single-user helpers move: `create_malformed_inputs`, `assert_error_contains_any`, `build_security_test_arguments` and `test_path_security_for_op` to `security`; the ten stress and spawn helpers to `concurrency`; `test_read_with_offset_limit` to `read`; `extract_text_content` to `properties`; `profile_memory` to `performance`.

    `model.rs` — both test modules move. `mod tests` becomes `model/tests.rs` plus 8 modules: `serialization`, `parsing`, `manager`, `resolution`, `paths`, `loading`, `config_structure`, `types`. `mod chat_model_config_tests` keeps its name and becomes `model/chat_model_config_tests.rs`. The parent keeps `embedding_model_config`, which `serialization` and `types` both use.

    `context.rs` — the test block becomes `context/tests.rs` plus 8 modules: `basics`, `changelog`, `attachments`, `enrichment`, `archive`, `error_paths`, `computed`, `cache`. The parent keeps `ctx_with_tag_store` (changelog and archive) and `attachment_fields_context` (attachments, enrichment and computed). `count_entity_format_lines` has one user and moves with it, as do the seven compute-engine fixtures and the two cache fixtures.

    **The hazard the first pass recorded, measured on these four.** The split tool never re-indents a line that begins inside a string literal, so it cannot corrupt a fixture. The count of such lines:

    | file | lines that begin inside a string | of those, at column 0-3 |
    | --- | --- | --- |
    | `complexity.rs` | 1342 | 489 |
    | `model.rs` | 232 | 148 |
    | `context.rs` | 0 | 0 |
    | `file_tools_integrations.rs` | 0 | 0 |

    The 489 lines in `complexity.rs` are the Python, Ruby and Fortran fixture sources, where the indentation is the syntax. A blind 4-space cut would have changed 637 fixture lines across the two files.

    **One defect found, and left.** `profile_memory` in `file_tools_integrations.rs` carries `#[allow(dead_code)]` and has no caller. It moved with `MemoryProfiler` to `performance.rs` unchanged. A separate card records it.
  timestamp: 2026-08-13T18:54:54.850217+00:00
- actor: claude-code
  id: 01kzy7n754f3h5rs23qpzgnz91
  text: |
    **Proof that the move is pure.** Two stages, not a claim.

    Stage 1, before `cargo fmt`: the split tool recorded, for every line it wrote, the source line it came from and the number of columns it removed. A second script read the files back off disk, put those columns back, placed each line at its original number, and compared the result with `git show HEAD:<file>`.

    ```
    PURE MOVE: crates/swissarmyhammer-tools/tests/integration/file_tools_integrations.rs reassembles byte-identical (4637 lines, 97 scaffolding lines added, 0 removed)
    PURE MOVE: crates/swissarmyhammer-entity/src/context.rs reassembles byte-identical (4175 lines, 70 scaffolding lines added, 2 removed)
      scaffolding removed: line 1812 'mod tests {'; line 4175 '}'
    PURE MOVE: crates/swissarmyhammer-config/src/model.rs reassembles byte-identical (4479 lines, 85 scaffolding lines added, 4 removed)
      scaffolding removed: line 1634 'mod tests {'; line 4270 '}'; line 4282 'mod chat_model_config_tests {'; line 4479 '}'
    PURE MOVE: crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs reassembles byte-identical (5520 lines, 84 scaffolding lines added, 2 removed)
      scaffolding removed: line 2238 'mod tests {'; line 5520 '}'
    ```

    The removed lines are the `mod ... {` headers and their closing braces, which the `mod ...;` declarations replace. The added lines are the module docs, the `mod` lists and the `use super::*;` lines.

    Stage 2, after `cargo fmt`: the moved lines lost four columns of width, so rustfmt re-wrapped some of them. A copy of the stage-1 tree was compared with the formatted tree two ways — every string literal byte for byte and in order, then the rest of the source with all whitespace collapsed and every trailing comma removed. Result: `47 files compared, 0 differ`. The formatter changed line breaks only, and no fixture moved a byte.

    **Test count, for each file, before and after.**

    | file | before | after |
    | --- | --- | --- |
    | `complexity.rs` | 116 | 116 |
    | `file_tools_integrations.rs` | 96 | 96 |
    | `model.rs` | 142 | 142 |
    | `context.rs` | 78 | 78 |

    Counted with `cargo nextest list` on the module prefix, on the same commit before and after the split.

    **Floor bytes against the 262144 cap.** 47 files, the largest at 55.4 percent of the cap. The three production files are the largest, and each is now the production code alone.

    | floor | raw | lines | % of cap | file |
    | --- | --- | --- | --- | --- |
    | 145299 | 94363 | 2238 | 55.4 | `swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` |
    | 118064 | 76500 | 1812 | 45.0 | `swissarmyhammer-entity/src/context.rs` |
    | 99100 | 61188 | 1646 | 37.8 | `swissarmyhammer-config/src/model.rs` |
    | 42762 | 24914 | 734 | 16.3 | `swissarmyhammer-config/src/model/tests/manager.rs` |
    | 36258 | 20896 | 621 | 13.8 | `.../file_tools_integrations/concurrency.rs` |
    | 33155 | 19795 | 530 | 12.6 | `.../file_tools_integrations/security.rs` |
    | 32242 | 18970 | 526 | 12.3 | `swissarmyhammer-entity/src/context/tests/computed.rs` |
    | 31271 | 18703 | 494 | 11.9 | `.../file_tools_integrations/grep.rs` |
    | 30783 | 18017 | 503 | 11.7 | `.../file_tools_integrations/read.rs` |
    | 30512 | 18208 | 482 | 11.6 | `.../file_tools_integrations/edit.rs` |

    The remaining 37 files sit between 2.0 and 10.7 percent of the cap.

    **No source file in the repository is now over the cap.** A sweep of `git ls-files` finds 64 files whose floor passes 262144. Every one is a binary asset (`.png`, `.icns`, `Assets.car`), a lock file (`Cargo.lock`, `package-lock.json`), or kanban board data (`.kanban/**/*.jsonl`). None is source the review engine reads.

    **Verification.**
    - `cargo nextest run -p swissarmyhammer-sem -p swissarmyhammer-config -p swissarmyhammer-entity -p swissarmyhammer-tools`: 2658 tests run, 2658 passed, 0 skipped.
    - `cargo nextest run --workspace`: 14181 tests run, 14181 passed, 0 skipped.
    - `cargo fmt --all --check`: clean.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean.

    **One thing the next agent should know.** The first workspace run reported 5 timeouts, all in `swissarmyhammer-validators review::tool_rules::tests::shipped::{magic_numbers,missing_docs}` for Dart. They are unrelated to this change: the crate does not depend on the four files. Run on their own they take 46 to 58 seconds each and all pass; under full workspace load with a cold Dart tool cache they passed the 300 second limit. The second workspace run, with the cache warm, was green.
  timestamp: 2026-08-13T18:55:21.764219+00:00
- actor: claude-code
  id: 01kzy7nz4s9kavfsj319dfj3ve
  text: |
    ### implement — changed
    - evidence: 47 files — 4 sources truncated to production code plus a `mod ...;` declaration, and 43 new modules. `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` + `complexity/tests.rs` + 13 language modules; `crates/swissarmyhammer-tools/tests/integration/file_tools_integrations.rs` + 10 subject modules; `crates/swissarmyhammer-config/src/model.rs` + `model/tests.rs` + 8 modules + `model/chat_model_config_tests.rs`; `crates/swissarmyhammer-entity/src/context.rs` + `context/tests.rs` + 8 modules. Pure move proved twice: byte-identical reassembly against `git show HEAD`, then a string-literal and whitespace-normalised comparison across `cargo fmt` (47 files, 0 differ). Test counts held: 116, 96, 142, 78. Largest floor 145299 of the 262144 cap, 55.4 percent. `cargo nextest run --workspace` 14181 run, 14181 passed, 0 skipped. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: every file named on this card is now split. No source file in the repository passes the per-file cap. New card ^pap9haz records the dead `profile_memory` helper found during the move. The card stays in `doing` for review.
  timestamp: 2026-08-13T18:55:46.329192+00:00
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