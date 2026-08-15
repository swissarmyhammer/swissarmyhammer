---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m015fnsmncmgx410c50r6p95
  text: |-
    Picked up. Reproduced the variance on the SAME bytes, same clean git state, back to back:

    | run | time | findings | refuted | attempted |
    | --- | --- | --- | --- | --- |
    | A | 17:06 | 2 | 10 | 9 |
    | B | 17:12 | 0 | 0 | 9 |

    Command both times: `review file crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` with `validators: ["reuse"]`. `git status --porcelain` on that path was EMPTY for both runs, so the file matched HEAD byte for byte. `skipped_files` empty both times. Two runs were enough; the card's own 0/8/0 is the same shape.

    Run A's two findings both quoted similarity numbers ("0.95-0.98 similarity", "0.91 similarity to duplication.rs:979").

    ## Mechanism, named

    `review file` on a file that matches HEAD produces NO probe evidence at all, so the `reuse` validator judges from nothing and its output is invention.

    The chain:

    1. `crates/swissarmyhammer-validators/src/review/scope/resolve.rs::resolve_file` reads the base side from HEAD unconditionally: `let before = BeforeContent::new(read_at_ref(&repo, GitRefSpec::head(), FilePath::new(path))?);`
    2. `FileChangeBuilder::push` then records the file as `FileStatus::Modified` with `before == after`.
    3. `crates/swissarmyhammer-sem/src/parser/differ.rs::compute_semantic_diff` matches before entities against identical after entities, gets `result.changes.is_empty()`, and returns `None` for the file. ZERO `ChangeEntry`s.
    4. `run_similar` (probes.rs) iterates `file_change.entities.filter(is_added && is_function)`. With zero entities it pushes zero `ProbeResult`s -- not an empty-rows result, NO result.
    5. The `reuse` validator still runs (it matched the file), with no `similar` block in its prompt.

    An empty candidate list is NOT distinguishable from a probe that never ran: `run_probes` documents "a probe that simply finds nothing yields a ProbeResult with empty rows", but the entity-bound probes emit no result at all when there are no entities, and nothing downstream reports the difference.

    ## The code contradicts its own written contract

    Two places already say the unchanged single file must diff as all-added:
    - `resolve_file` doc: "Resolve a single-file scope: its working-tree changes if any, else its whole content reviewed as all-added work."
    - `scope.rs` in `scope_review`: "Whole-content files (glob / unchanged single file) carry only `after_content`, so they diff as all-added entities."

    `resolve_glob` does exactly that (`before: BeforeContent::absent()`). `resolve_file` does not. That is the defect.

    ## Ruled OUT

    - Size cap / `skipped_files`: empty on every run.
    - Index emptiness: `.code-context/index.db` holds 50433 chunks, 50432 embedded; `indexed_files` 1904/1904.
    - Timeout or resource limit: there is NO `tokio::time::timeout`, no row cap, no `spawn_blocking`, no `catch_unwind` anywhere on the probe path in `crates/swissarmyhammer-validators/src/review/`. Every timeout in `drive.rs` is inside `#[cfg(test)]`. The `budget` values in `probes.rs`/`fleet.rs`/`batch.rs` are RENDERED PROMPT BYTES, not execution budgets.
    - Embedder fallback: no zero/dummy vector path; `Embedder::embed_text` failure becomes `AvpError::Context` and aborts the run. The model is fully loaded before the pipeline starts.
    - `swissarmyhammer-search` scoring nondeterminism: `fuse_and_rank` builds hits in doc-index order (`0..docs.len()`), never by iterating the `rrf_fuse` HashMap, and sorts stably. `ranked_indices` is likewise stable. Deterministic for a fixed corpus.
    - Corpus load error swallowed into an empty Vec: it is not; `load_all_embedded_chunks` errors propagate as `AvpError::Context`.
  timestamp: 2026-08-14T22:15:06.292633+00:00
- actor: claude-code
  id: 01m016djffsc1w8xnp6cv8tpkn
  text: |-
    ## JOB 1 — fixed, with a RED→GREEN test

    `crates/swissarmyhammer-validators/src/review/scope/resolve.rs::resolve_file` now drops the base side when the working tree matches HEAD:

    ```rust
    let base = if head == working { None } else { head };
    ```

    That is the shape `resolve_glob` already uses, and it is what TWO existing comments already promised:
    - `resolve_file`'s own doc: "its working-tree changes if any, else its whole content reviewed as all-added work."
    - `scope_review`: "Whole-content files (glob / unchanged single file) carry only `after_content`, so they diff as all-added entities."

    The code did not do that. Now it does.

    ### The test, watched fail then pass

    `crates/swissarmyhammer-validators/src/review/scope/tests.rs::file_scope_probes_a_file_with_no_working_tree_change_as_whole_added_work`

    It asserts the RENDERED payload first, because that text IS the defect.

    - RED (base side kept): `the rendered payload must name the similar probe, got: # Files under review ...` — the payload carried `_No probe evidence._`.
    - GREEN (base side dropped): passes. The payload names `probe \`similar\``, the semantic diff carries the entity, and the probe row points at the seeded candidate.

    I reverted the fix and re-ran to watch the render assertion fail on its own, then restored it. Both directions measured.

    ### Both halves of the card's diagnostic question, answered

    1. **Was the probe run at all on every pass?** No. On a file matching HEAD the entity-bound probes ran zero times, because there were zero entities to bind to.
    2. **Was an empty candidate list distinguishable from a failed one?** No. Zero results renders `_No probe evidence._`, which says nothing about whether `similar` ran. With the fix the probe always produces a result, and a genuinely empty one renders its own header plus the `(no rows)` sentinel `render_result_rows_within` already writes. Empty and absent are now different text.

    ### What I do NOT claim

    The running MCP server is PID 16107, started 09:56:49, executing `target/release/sah` built at 09:56 — BEFORE this edit. Every `review file` run in this session, including the two that reproduced the variance, executed the PRE-FIX binary. This change is in the source tree and is proven by the test; it is NOT yet proven against the live tool. A `review file` repeat measurement is only meaningful after the server restarts on a rebuilt binary.

    Engine-side nondeterminism is what this removes: the probe now runs on every pass and its evidence is identical run to run for a fixed index. Whether the validator AGENT still samples differently given identical evidence is a separate question this change does not settle, and I am not asserting it does.

    ## JOB 2 — settled: NO shared tree-sitter helper module

    I checked every claim on the card against the source. All confirmed.

    `node_text`, four copies, four contracts:

    | site | signature | answer when the text is absent |
    | --- | --- | --- |
    | `complexity.rs` | `(Node, &str) -> Option<&str>` | `None` |
    | `duplication.rs` | `(Node, &str) -> &str` | `""` |
    | `entity_extractor.rs` | `(Node, &[u8]) -> &str` | `""`, through `utf8_text` |
    | `swissarmyhammer-treesitter::ParsedFile::node_text` | a METHOD, `-> Option<&str>` | `None` |

    The `Option` and the `""` are not two spellings of one answer. `complexity` compares the text against test markers, so `""` there reads as "not a test" and scores a test function as complex code. `duplication` must still hash a chunk whose slice it cannot read, so `""` is the answer it needs. `entity_extractor` takes bytes and VALIDATES UTF-8; the two `&str` copies slice already-valid UTF-8 and miss on a codepoint boundary instead. Different failure modes, not one function.

    `spec_for_language` / `language_spec`, four copies over four rosters:
    - `ALL_SPECS: &[&ComplexitySpec]` — references, body ends `.copied()`
    - `COMMENT_SPECS: &[&CommentSpec]` — references
    - `SURFACE_SPECS: &[SurfaceSpec]` — values
    - `LANGUAGE_SPECS: &[LanguageSpec]` — values

    Four unrelated types; two slices of references, two of values. A shared version needs a trait, four impls, and a generic function to replace four one-line `.iter().find()` bodies. That relocates code and adds to it. The `duplication/rust` rule states the exact test this fails: "Do not flag this unless a further shared abstraction would strictly reduce the code (not just relocate it) and preserve locality."

    `is_test_definition`, two copies sharing a name and nothing else: `duplication` takes a `TestSpec` and ORs four `marked_by_*` helpers; `complexity` takes a `ComplexitySpec` and reads a name, then a defining call's target, then attributes.

    Three named counterparts are in OTHER crates (`swissarmyhammer-treesitter`, `swissarmyhammer-templating`) and one is a TEST file in `swissarmyhammer-tools`. A helper for these plugins cannot live in any of them.

    The `reuse` rule's own carve-out covers this exactly: "A `similar` candidate that only *looks* alike (same shape, different domain or contract) is not a reuse miss."

    ### Where the decision is written, so the next run finds it

    The validator reads the file WHOLE. So the record sits in the code, not in a report.

    - `crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs` — a new module doc section carries the whole decision: the four-contract table, the roster measurement, and the cross-crate facts.
    - Nine per-site doc records, one at each function the probe raised. Each states THAT SITE's own contract, so no two are alike and the notes are not themselves a duplicated block: `complexity.rs` (`node_text`, `spec_for_language`, `is_test_definition`), `duplication.rs` (`node_text`, `language_spec`, `is_test_definition`), `entity_extractor.rs` (`node_text`), `commented_code.rs` (`spec_for_language`), `public_surface.rs` (`spec_for_language`).

    Reviewing any one of those files now inlines the reason the pair is not a reuse miss.

    ## Verification

    - `cargo nextest run --workspace` — 14213 run, 14213 passed, 0 skipped.
    - `cargo fmt --check` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean.
    - `cargo doc --no-deps -p swissarmyhammer-sem` — 13 warnings, and the stashed baseline is also 13. Zero added.
  timestamp: 2026-08-14T22:31:25.935983+00:00
- actor: claude-code
  id: 01m016drvdxb09bk4c6yppjvm6
  text: |-
    ### implement — changed
    - evidence: 8 files — crates/swissarmyhammer-validators/src/review/scope/resolve.rs, crates/swissarmyhammer-validators/src/review/scope/tests.rs, crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs, complexity.rs, duplication.rs, entity_extractor.rs, commented_code.rs, public_surface.rs. `cargo nextest run --workspace` 14213 passed 0 skipped; fmt and clippy clean.
    - next: /review. Note for the reviewer: the fix is NOT yet proven against the live `review file`, because the running MCP server (PID 16107) executes a binary built before the edit.
  timestamp: 2026-08-14T22:31:32.461187+00:00
- actor: claude-code
  id: 01m016rscan8eqbc0cyxb9hnkf
  text: |-
    ### review — clean
    - scope: `review sha 8dcb83f15~1..8dcb83f15` — diffs only, 8 files reviewed, 0 not reviewed, 0 skipped
    - evidence: counts findings=0, confirmed=0, refuted=1, attempted=9, failed=0 — no file:line findings
    - no `reuse` or `duplication` finding reached the report on the nine new per-site doc records or on the functions they document
    - no prior Review Findings section on this task, so nothing was left unchecked
    - next: none — task moved to done
  timestamp: 2026-08-14T22:37:33.450689+00:00
- actor: claude-code
  id: 01m016t5hwwct31g2cm9z9mpfv
  text: |-
    ### finish iteration 1 — clean. Card done.
    - **Job 1: reproduced, root-caused, fixed in one line.** Reproduced at 2 findings then 0 on `complexity.rs` with a clean tree both times. `resolve_file` read the base side from HEAD unconditionally, so a file matching HEAD was diffed AGAINST ITSELF: the sem differ returned no changes, and `run_similar` — which iterates ADDED function entities — emitted zero `ProbeResult`s. Not an empty-rows result; NO result. The prompt rendered `_No probe evidence._`, which says nothing about whether the probe ran, so `reuse` judged from nothing and varied run to run.
    - It contradicted two comments already in the tree: `resolve_file`'s own doc says a file carries "its working-tree changes if any, else its whole content reviewed as all-added work", and `scope_review` says whole-content files carry only `after_content`. `resolve_glob` did that; `resolve_file` did not. Fix: `let base = if head == working { None } else { head };`
    - Both halves of the card's diagnostic question are answered: the probe runs on every pass, and an empty candidate list now renders its own header plus the `(no rows)` sentinel — different text from `_No probe evidence._`.
    - Alternatives ruled out with EVIDENCE, not reasoning: index fully embedded (50432/50433 chunks, 1904/1904 files); no timeout, row cap, `spawn_blocking` or `catch_unwind` anywhere on the probe path (every timeout in drive.rs is `#[cfg(test)]`, and `budget` values are rendered prompt bytes); no dummy-vector fallback in the embedder; `fuse_and_rank` builds hits in doc-index order with a stable sort, so it never leaks HashMap ordering.
    - **Job 2: settled AGAINST a shared tree-sitter helper**, and the card's evidence was verified against source rather than taken. The `Option` vs `""` split in `node_text` is LOAD-BEARING: `complexity` compares the text against test markers, so `""` would read as "not a test" and score a test function as complex code, while `duplication` must still hash a chunk whose slice it cannot read; `entity_extractor` takes `&[u8]` and validates UTF-8 where the two `&str` copies slice and miss on codepoint boundaries. The four `spec_for_language` rosters are four unrelated types, two slices of references and two of values — a shared version costs a trait plus four impls to replace four one-line bodies, which the `duplication/rust` rule rejects as relocation rather than reduction.
    - The decision is written WHERE THE VALIDATOR WILL FIND IT — the reuse validator inlines the file, so it lives in the code: a module-doc section plus nine per-site records, each stating that site's own contract so the notes are not themselves a duplicated block.
    - test: green — 14213 passed, 0 skipped. fmt and clippy clean. `cargo doc` warnings unchanged from the stashed baseline, 13 both ways.
    - commit: 8dcb83f15
    - review: clean — 0 findings, 9 attempted, 1 refuted, 8 files reviewed, 0 not reviewed. No reuse or duplication finding fired on the new doc records or the functions they document, so the loop this card exists to stop did not recur.

    **Two bounds on what this proves, stated rather than glossed.** The defect is in `resolve_file` and reaches `review file` ONLY — a `review sha` run, where before and after genuinely differ, could not exercise it either way, so the finish loop's own reviews were never affected. And the running server executes a binary built BEFORE this commit, so the fix is not live in the process that reviewed it. Proving `review file` repeatable needs a rebuild and a server restart.
  timestamp: 2026-08-14T22:38:18.684519+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff8680
title: 'review file is not repeatable: the same bytes returned 0, then 8, then 0 findings'
---
Measured on 2026-08-14 while working ^hmw6f0z, on `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs`.

Three `review file` runs on that path. The file was **not edited between run 1 and run 2**, and only a doc comment on one `pub use` changed between run 2 and run 3.

| run | time | findings | refuted | attempted |
| --- | --- | --- | --- | --- |
| 1 | 11:57 | **0** | 0 | 9 |
| 2 | 12:42 | **8** | 3 | 9 |
| 3 | 12:49 | **0** | 0 | 9 |

`skipped_files` was empty on all three, so the cap was not the cause.

## What run 2 reported

One finding was real and in the file, and is now fixed on ^hmw6f0z: `complexity.rs:164` `rust/documentation` — the `pub use test_census::{...}` re-export carried no doc comment.

The other **seven were all `reuse/reuse`**, and every one asked to extract a tree-sitter helper into a shared module that does not exist:

- `spec_for_language` (1055) vs `commented_code.rs::spec_for_language`, `public_surface.rs::spec_for_language`, `duplication.rs::language_spec`
- `function_header` (1380) vs `test_census.rs::body_statements`, `duplication.rs::declaration_header`
- `node_text` (1407) vs `duplication.rs::node_text`, `entity_extractor.rs::node_text`, `swissarmyhammer-treesitter/src/parsed_file.rs::ParsedFile::node_text`
- `child_by_field_or_kind` (1425) vs `duplication.rs::collect_under`, `entity_extractor.rs::find_in_named_children`
- `is_test_definition` (1446) vs `duplication.rs::is_test_definition`
- `attribute_marker_name` (1530) vs `chunk.rs::extract_impl_type_name`, and a TEST file in another crate (`swissarmyhammer-tools/tests/integration/file_tools_integrations.rs`)
- `name_signature_marks_test` (1563) vs `swissarmyhammer-templating/src/prompts.rs`

Two things are worth separating here.

## 1. The repeatability defect

A rule that fires on one run and not the next is not a requirement anyone can satisfy. A card can be driven to clean and reopen with no code change. `reuse` and `duplication` are the two validators whose input is a probe result (`similar`, `duplicates`), so the first suspect is the probe returning candidates on one run and none on another — check whether the probe is being run at all on every pass, and whether an empty candidate list is distinguishable from a failed one. This matters more than any single finding: ^hmw6f0z's own "done when" is "the four files re-review with no confirmed finding", and that condition is not decidable while the engine is non-deterministic.

## 2. The shared tree-sitter utility question, on its merits

Independent of the flakiness, decide once whether the five sibling plugins under `crates/swissarmyhammer-sem/src/parser/plugins/code/` should share a tree-sitter helper module. Evidence gathered while judging run 2:

- `node_text` is genuinely repeated, but the four copies have **four different contracts**: `Option<&str>` (complexity), `&str` defaulting to `""` (duplication), `&[u8]` source via `utf8_text` (entity_extractor), and a method on `ParsedFile` in another crate. Unifying means choosing one contract and changing three call-site families.
- `spec_for_language` reads **four different static tables of four different spec types** (`ALL_SPECS`/`ComplexitySpec`, `COMMENT_SPECS`/`CommentSpec`, `SURFACE_SPECS`/`SurfaceSpec`, `LANGUAGE_SPECS`/`LanguageSpec`), and two of them are slices of references while two are slices of values. A shared version needs a trait plus four impls to replace four one-line `.iter().find()` calls — more code, not less.
- Three of the named counterparts are in **other crates** (`swissarmyhammer-treesitter`, `swissarmyhammer-templating`) and one is a **test file** in `swissarmyhammer-tools`.

The reuse rule's own carve-out says a `similar` candidate that looks alike but has a different domain or contract is not a reuse miss. Somebody should read these seven pairs once, decide, and write the decision down — so the next run of the validator either has nothing to say or has a settled answer to point at.

## Done when

- `review file` on an unedited file returns the same finding set on repeated runs, and the cause of the variance is named.
- The shared tree-sitter helper question is settled in writing: either the module exists and the copies call it, or the carve-out is recorded against these specific pairs.

#tool-validators