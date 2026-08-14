---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m00jc04b9erjesfxhjfvrs0m
  text: |-
    Picked up. Moved to doing.

    Plan per instruction: STEP 1 re-measure with `review file` on all four files before touching anything. The recorded 28 findings are stale on two counts — measured with the old whole-file engine (before ^apb04az), and measured while `duplication/duplication-parsed` and `code-hygiene/no-commented-code-parsed` reported `tool missing: bash: : command not found` so prompt rules substituted. Those two stale rules are now gone from `~/.validators/`, so the deterministic duplication detector runs this time. STEP 2 works the CURRENT list, not the recorded one.
  timestamp: 2026-08-14T16:41:02.859405+00:00
- actor: claude-code
  id: 01m00pny2d9j4wr7zc73fjpbm1
  text: |-
    ## STEP 1 — re-measurement, before any edit

    `review file` on each of the four files. `skipped_files` was empty on all four, so every file was read whole.

    | file | recorded | current | attempted |
    | --- | --- | --- | --- |
    | `swissarmyhammer-entity/src/context.rs` | 9 | **16** | 9 |
    | `swissarmyhammer-config/src/model.rs` | 11 | **11** | 9 |
    | `swissarmyhammer-sem/.../complexity.rs` | 7 | **0** | 9 |
    | `swissarmyhammer-kanban/src/dispatch.rs` | 1 | **3** | 9 |
    | total | 28 | **30** | |

    **5 of the recorded 28 persist. 23 are gone. 25 are new.** The two lists barely overlap.

    Persisting:
    - `model.rs:349,351` — missing docs on the two `ModelExecutorConfig` variants, word for word the same finding.
    - The `context.rs` restore/delete duplication family. Five recorded findings (371, 438, 454 twice, 471) are now three (471, 487, 540), same causes, re-anchored.

    Gone — and the reason matters:
    - **All 15 cross-file "extract to a shared module" findings vanished** (`model.rs` 648/915/953/1008/1461, all 7 on `complexity.rs`, `dispatch.rs:750`). These were produced by the PROMPT duplication rule standing in for the missing `duplication-parsed` tool rule. With the stale rules pruned from `~/.validators/`, the deterministic detector runs and finds nothing across crates.
    - `context.rs:50` "should implement Debug" — gone.
    - The three `context.rs` path-traversal findings (180, 222, 742) — gone. `io::entity_file_path` already routes ids through `sanitize_id`.
    - `model.rs:482` "io error lowercases the acronym" — gone, and correctly: the error-handling rule says do not change a first word that IS an all-caps acronym; it does not require capitalizing an already-lowercase one.
    - `model.rs:1493,1506` `get_` prefix on `get_chat_model` / `get_review_chat_model` — gone. Neither is a getter; both take a `paths` argument and read config from disk.

    New, and dominant: `rust/type-safety` (12), `rust/trait-implementations` (4), `rust/documentation` (4), `naming/naming-consistency` (2), plus the `dispatch.rs` `tag_refs` / `explicit_assignee_refs` pair. The old engine never reported one of these on these files.
  timestamp: 2026-08-14T17:56:22.733674+00:00
- actor: claude-code
  id: 01m00pppyh25dr9xc5wqn6t78r
  text: |-
    ## STEP 2 — what each cause was, and every extra site the sweep found

    ### `context.rs` — 16 findings, 2 causes

    **`rust/type-safety`, 11 findings.** Every public method taking `(entity_type, id)` declared both as `impl AsRef<str>`, so the two are interchangeable at a call site. The crate already owns the right newtypes — `swissarmyhammer_fields::EntityTypeName` and `crate::id_types::EntityId` — and `Entity::new(entity_type: impl Into<EntityTypeName>, id: impl Into<EntityId>)` is the prevailing signature for exactly this pair. Every flagged method now matches it. **The sweep found 1 extra site the review did not name: `sync_entity_cache_from_disk`, which took `(&str, &str)`.** 12 methods converted.

    Single-`entity_type` methods (`list`, `list_archived`, `entity_dir`, `entity_def`, `trash_dir`, `archive_dir`, `migrate_trash_layout`) were left on `impl AsRef<str>`: one parameter cannot be swapped with another, so the cause is absent, and `entity_def`/`entity_dir` are on every internal read path where `Into` would cost an allocation per call.

    **`duplication/duplication`, 3 findings** (471, 487, 540). Three separate copy families, all now one body each:
    - `delete_internal` / `archive_internal` / `unarchive_internal` → a `StagingOp { Delete, Archive, Unarchive }` discriminant with `stage_internal` + `stage_fallback`.
    - The public `delete` / `archive` / `unarchive` cache-routing trio → `stage`. **Not flagged in this run, but the same cause, and finding 371 of the recorded 28 named it.**
    - `restore_from_trash_internal` / `restore_from_archive_internal` → a `StagingDir { Trash, Archive }` discriminant with `restore_internal`; the public pair → `restore`.

    The two `pub(crate)` `restore_from_*_internal` methods had no caller outside this file and were deleted rather than kept as shims.

    **`rust/documentation`, 2 findings** (1003, 1004): the two bare `.unwrap()`s in `validate_for_write`, now `.expect()` naming the invariant. These were the only two `.unwrap()`s in the file.

    ### `model.rs` — 11 findings, 4 causes

    - **`rust/trait-implementations`, 4 findings** (125, 257, 271, 357): `PartialEq, Eq` added. **2 extra sites: `ModelExecutorConfig` (which `ExecutorEntry` needs to derive at all) and `ModelManager`.**
    - **`naming/naming-consistency`, 2 findings** (191, 193): `MacosX86_64` → `MacosX8664`, `LinuxX86_64` → `LinuxX8664`. `Platform` is `serde(rename_all = "kebab-case")` and `platform:` is written into user model YAML, so the rename would silently change the wire names. Both variants now carry an explicit `#[serde(rename = "macos-x86-64")]` / `"linux-x86-64"`, and a new test `platform_wire_names_are_stable` pins all four wire names in both directions. Written and run BEFORE the rename to prove the strings.
    - **`rust/documentation`, 2 findings** (279, 492): doc comments on the two hand-written trait impls. Those are the only two in the file.
    - **`code-hygiene/missing-docs-rust`, 2 findings** (349, 351) and **`rust/type-safety`, 1** (1571): `review_chat_model_from(Option<String>, Option<String>)` now takes `ReviewModel` and `DefaultModel`.

    ### `dispatch.rs` — 3 findings, 1 cause

    `tag_refs` and `explicit_assignee_refs` were the same plural-then-singular fallback with different keys. Extracted `aliased_list_param(op, plural, singular)`. **0 extra sites — those are the only two alias pairs in the file; `depends_on` has no alias.**

    ### One enabling change outside the four files

    Converting the public API to `impl Into<EntityTypeName>` / `impl Into<EntityId>` broke **89 call sites**, almost all of the form `ctx.read("task", &self.id)` where the field is a `String`. `&String` does not reach `From<&str>`: deref coercion applies to a declared `&str` parameter, never to an `Into<T>` bound.

    Rather than scatter `.as_str()` across 89 sites in 20 files (most of them existing tests), one `impl From<&String>` was added to `define_id!` in `swissarmyhammer-common` — the canonical macro that builds all ~15 id newtypes. That dropped the breakage to zero. Written test-first: `entity_id_from_string_reference` failed to compile with "the trait `From<&String>` is not implemented", then passed. The first approach — editing all 89 call sites — was tried, measured, and reverted; the reverted files are listed nowhere in the final diff.
  timestamp: 2026-08-14T17:56:48.209871+00:00
- actor: claude-code
  id: 01m00pr4ddxgatrnbrdzw09e71
  text: |-
    ## Verification, and one thing the verification exposed

    Final `review file` on all four files: **0 confirmed findings each**, `skipped_files` empty on every run.

    - `context.rs` 16 → 0
    - `model.rs` 11 → 0 (a first re-run returned 2 `rust/documentation` findings on the two `From` impls I had just added for `ReviewModel`/`DefaultModel`; both now carry doc comments)
    - `dispatch.rs` 3 → 0
    - `complexity.rs` 0 → 0

    **But `complexity.rs` is not a trustworthy 0.** Three runs on that path returned 0, then 8, then 0, with the file unedited between the first two. Of the 8, one was real and in the file (`:164`, an undocumented `pub use`) and is fixed here. The other seven were all `reuse/reuse` findings pointing at other files and, in three cases, other crates. Full measurement and the merits of the shared-tree-sitter-helper question are on the new card **^4dyewvd** — including why `node_text`'s four copies have four different contracts and why a shared `spec_for_language` would need a trait plus four impls to replace four one-line `.iter().find()` calls.

    This matters for this card's "done when": *the four files re-review with no confirmed finding* is not a decidable condition while the engine gives a different answer to the same bytes. It is satisfied on the current measurement; it may not be on the next one, through no change to the code.

    ## Not changed, and why

    - `get_chat_model` / `get_review_chat_model` keep their `get_` prefix. The api-design rule bans it on getters; neither is a getter — both take a `paths` argument and read config off disk. Recorded findings 1493/1506 did not recur.
    - `#[error("io error: {0}")]` in `ModelError` is unchanged. Recorded finding 482 read the error-handling rule backwards: it says do not change a first word that IS an all-caps acronym, not that a lowercase one must be capitalized. Did not recur.
    - `trash_removed_attachments(&self, entity_type: &str, old: &Entity, new: &Entity)` has two same-typed parameters, but `&Entity` is not a primitive and old-vs-new is the function's whole purpose. Not flagged, not changed.
    - `pub(crate)` internals in `context.rs` stay on `&str`. Across eleven opportunities the type-safety rule flagged only `pub` methods; it lives in the `rust` validator, which is described as public-API review guidance. Converting them would push the change into `cache.rs`, `watcher.rs`, and `undo_commands.rs`, which this card does not scope.

    ## Gate results

    - `cargo nextest run --workspace` — **14199 passed, 0 failed, 0 skipped**
    - `cargo fmt --all --check` — clean
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean
    - Per-package: `swissarmyhammer-common`, `-fields`, `-entity`, `-config`, `-kanban`, `-sem` — 3370 passed, 0 skipped
  timestamp: 2026-08-14T17:57:34.765759+00:00
- actor: claude-code
  id: 01m00prcx7vrq7wmxkw5v5cgag
  text: |-
    ### implement — changed
    - evidence: 9 files — crates/swissarmyhammer-entity/src/context.rs, crates/swissarmyhammer-entity/src/id_types.rs, crates/swissarmyhammer-common/src/id_types.rs, crates/swissarmyhammer-config/src/model.rs, crates/swissarmyhammer-config/src/model/tests/types.rs, crates/swissarmyhammer-config/src/model/tests/resolution.rs, crates/swissarmyhammer-config/src/model/chat_model_config_tests.rs, crates/swissarmyhammer-kanban/src/dispatch.rs, crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs. Re-measured first: 30 current findings against the recorded 28, 5 persisting. All 30 worked, plus 4 extra sites the sweep found. All four files re-review at 0. cargo nextest run --workspace 14199 passed / 0 skipped; fmt and clippy clean.
    - next: /review. Engine repeatability is raised on ^4dyewvd — complexity.rs returned 0, 8, 0 on the same bytes, so this card's "four files re-review clean" holds on the current measurement but is not a stable condition.
  timestamp: 2026-08-14T17:57:43.463468+00:00
- actor: claude-code
  id: 01m00q2pxgt77989750bvws8je
  text: |-
    ### test — green
    - evidence: `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo nextest run --workspace` — 14199 passed, 0 failed, 0 skipped
    - verified the three flagged risk areas: shared `define_id!` macro's new `From<&String>` impl (single definition in swissarmyhammer-common/src/id_types.rs, reused by swissarmyhammer-entity/src/id_types.rs, no ambiguity/build errors), `Platform::MacosX8664` wire format pinned and round-trip tested in crates/swissarmyhammer-config/src/model/tests/types.rs (`platform_wire_names_are_stable`), and the `StagingOp`/`StagingDir` consolidation in crates/swissarmyhammer-entity/src/context.rs covered by the passing suite
    - next: hand off to review
  timestamp: 2026-08-14T18:03:21.392084+00:00
position_column: doing
position_ordinal: '8280'
title: 28 pre-existing findings in the six files ^0fn6dbf made reviewable
---
Splitting the six over-cap files on ^0fn6dbf made them readable by the review engine for the first time. The first narrow review of them returned **28 confirmed findings**, every one in PRE-EXISTING production code rather than in that card's own change. The new module scaffolding produced no surviving finding.

These are recorded here rather than on ^0fn6dbf because they are not that card's deliverable. ^0fn6dbf set out to bring the files under the 262144-byte cap, and it did — `skipped_files` was empty on all 11 review runs. Finding these defects is the POINT of that work, not a defect in it.

## The findings, by file

`crates/swissarmyhammer-entity/src/context.rs` — 50, 180, 222, 371, 438, 454 (two), 471, 742
`crates/swissarmyhammer-config/src/model.rs` — 349, 351, 482, 648, 915, 953, 1008, 1030, 1461, 1493, 1506
`crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` — 1171, 1225, 1332, 1518, 1563, 2119, 2129
`crates/swissarmyhammer-kanban/src/dispatch.rs` — 750

The full text of each stands in the `## Review Findings` section of ^0fn6dbf, dated 2026-08-14.

## Verified, not assumed

The reviewer read every asserted line before recording. **0 findings were dropped for a false premise** — including the two "undocumented enum variant" claims at `model.rs:349,351`, where both variants genuinely carry no doc comment, and the "should implement Debug" claim at `context.rs:50`, where no `#[derive(Debug)]` is present. A further **18 findings were dropped** under the review skill's existing-tests exception, each confirmed present verbatim before the split with `git show <sha>^`.

## One caveat on the measurement

On all 11 runs the tool rules `duplication/duplication-parsed` and `code-hygiene/no-commented-code-parsed` reported `tool missing: bash: : command not found`, so prompt rules substituted and the deterministic duplication detector never ran. Those two rules were deleted from `builtin/` on ^wwb6hk7 but survive in the deployed `~/.validators/` store; ^w6ypb8b prunes them, and takes effect after a `sah` rebuild plus `sah init user`. So a re-run after that cleanup may report a different duplication picture.

## Done when

- Each finding is judged on its own evidence, fixed or recorded as a conflict.
- The four files re-review with no confirmed finding.

#tool-validators