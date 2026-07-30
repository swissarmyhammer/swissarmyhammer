---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kysfvx9bftc42vwky8fv23wk
  text: |-
    Picked up by /finish #bug (task 1 of 3). No prior attempts on this card.

    Context from the session that filed it: the defect was found while tagging two other cards. `add task` with a `tags` array of full ULIDs returned `tags: []`. `update task` with a `tags` array of plain names returned `ok: true` and `get task` then showed `tags: []`. `tag task` with a singular `tag` worked every time. Both id forms failed, so resolution is not the cause.

    Note for whoever works this: `tag task` rejects a plural `tags` array loudly (`parse error: missing required field: tag`). That loud rejection is the correct behavior to copy — the silent path on add/update is the bug.
  timestamp: 2026-07-30T12:26:55.659838+00:00
- actor: claude-code
  id: 01kysgwjn27454hwqgys1kcfmf
  text: |-
    Research done. Root cause and audit results.

    ## Root cause

    Tags are NOT a stored field — they are derived by parsing `#tag` markers out of the task `body`. `AddTask` and `UpdateTask` (crates/swissarmyhammer-kanban/src/task/add.rs, update.rs) have NO `tags` field at all, and `dispatch.rs` never reads a `tags` param. So a `tags` array is dropped by serde's default unknown-field skipping plus a dispatch layer that never looks. Nothing was ever wired.

    `tag task` (task/tag.rs) has its own bespoke resolution: 26-char alnum -> read tag entity, else `normalize_slug`. On a ULID that does NOT exist as a tag entity it falls back to `normalize_slug(ulid)` and creates a tag literally named `01KJZ...` — a second silent-failure mode. `untag task` carries a copy of the same block.

    ## Sibling audit (step 4)

    - `depends_on` — CORRECT on both ops. `resolve_depends_on`/`depends_on_refs` in dispatch.rs already accept single ref / array / stringified array, resolve short ids, and error on unresolvable or malformed. This is the reference implementation to mirror.
    - `attachments` on `update task` — SAME DEFECT. `UpdateTask.attachments: Option<Value>` exists (so the full schema advertises it) but `dispatch_update_task` never reads an `attachments` param. 100% silent drop over MCP/dispatch. Only the Rust `with_attachments` builder works.
    - `assignees` on `add task` — PARTIAL DEFECT. `resolve_assignees` accepts a JSON array or the singular `assignee`, but silently drops a scalar string under `assignees`, a stringified array, and any malformed value (falls through to the actor fallback with no error).
    - `assignees` on `update task` — PARTIAL DEFECT. `.and_then(|v| v.as_array())` drops scalar/stringified silently, and `if !ids.is_empty()` means `assignees: []` cannot clear the list.
    - `attachments` on `add task` — not a field, not advertised. No defect.

    ## Plan

    New module `task/tags.rs` holding the ONE shared path: a pure `resolve_tag_ref` over the loaded tag entities plus `apply_tag_refs(ectx, entity, refs, mode)` with `Append`/`Replace`/`Remove`. `tag task`, `untag task`, `add task {tags}`, `update task {tags}` all route through it. Generalize dispatch's `depends_on_refs` into one `ref_list_param` shape normalizer reused by `tags`, `depends_on`, `assignees`, and `attachments`.
  timestamp: 2026-07-30T12:44:46.114557+00:00
- actor: claude-code
  id: 01kysnv10k6pyarmbkvg064jwq
  text: |-
    Implementation landed. Green.

    ## The one shared path

    New module `crates/swissarmyhammer-kanban/src/task/tags.rs`. It owns tag resolution and body rewriting for ALL FOUR ops that touch a task's tags: `tag task`, `untag task`, `add task {tags}`, `update task {tags}`. `TagApply::{Append, Replace, Remove}` selects the combining rule; everything else is identical, so the plural and singular forms cannot drift.

    `tag task` and `untag task` each carried a private copy of "26 chars alnum -> read tag entity, else normalize_slug". Both copies are gone.

    Resolution order for one ref: explicit id form (`^` sigil or full ULID) -> tag entity id, then tag NAME (so the legacy tags an earlier `tag task` created literally named `01KJZ...` stay reachable), else TagNotFound. Otherwise: existing tag name, then bare 7-char short id, else a new tag name created on demand. A ref with no slug characters is a parse error.

    Shape tolerance lives in dispatch. `depends_on_refs` became one generic `ref_list` + `list_param` used by `depends_on`, `tags`, `assignees`, and `attachments`, so all four accept single ref / array / stringified array and all four error on a malformed value instead of dropping it.

    ## Sibling audit results (step 4)

    - `depends_on` — already correct on both ops. It was the reference implementation.
    - `attachments` on `update task` — SAME DEFECT, now fixed. The field was declared on `UpdateTask` (so the schema advertised it) but dispatch never read it. 100% silent drop over MCP.
    - `assignees` on both ops — PARTIAL DEFECT, now fixed. A scalar string, a stringified array, and any malformed value were dropped silently, and `assignees: []` could not clear the list.
    - `assignees` unresolvable ACTOR ref — NOT fixed, its own card ^n36mc1q. `default_reference_validation` in swissarmyhammer-fields prunes dangling ids on write with no error, by documented policy, for every reference field. Changing that is wider than this card. The MCP description now states the difference plainly instead of implying `assignees` errors like `tags` does.
    - `attachments` on `add task` — not a field, not advertised, no defect.

    ## What the adversarial review caught (two rounds, both acted on)

    Round 1 found that my first cut was still broken in ways my own tests did not reach:

    1. `tag_parser::remove_tag` ended a match only at whitespace/`#`/EOL, but `parse_tags` ends a slug at ANY character outside `[A-Za-z0-9-]`. So `#bug,` read as the tag `bug` but could not be removed — `update task {tags: []}` was a silent no-op on such a body. This board carries several. Fixed with shared `slug_ends_at`/`slug_starts_at` helpers applied to both `remove_tag` and `rename_tag` (`rename_tag` had the identical bug, so tag rename was silently skipping those markers too).
    2. `append_tag` appends at the end of the last line. When that line is a code fence or a heading, `parse_tags` skips it, so the tag was written and then invisible — `ok` with `tags: []`. Fixed: `append_tag` escalates to its own line when an inline append does not read back, and `rewrite_body` verifies the round trip and returns an error rather than reporting a success the parser cannot see.
    3. `remove_tag`/`rename_tag` did not skip heading lines while `parse_tags` does, so replacing tags ate title text. Fixed.
    4. A tag ULID that names no tag entity but IS an existing tag's name could no longer be resolved, which made the legacy junk tags permanently unremovable. Fixed with the name fallback.
    5. `attachments` rejected the enriched metadata objects `get task` itself returns, breaking read-edit-write.
    6. No served-tool test, although the bug was filed at the MCP boundary.
    7. Singular `tag` on add/update was still a silent drop.

    Round 2 found three defects introduced BY those fixes, all fixed:

    A. `attachment_param` passed any all-object array through. An object without `id`/`name` resolves to nothing in the entity layer and vanishes — wiping the attachment list while reporting success. Now each element is validated, and mixed path/object lists are accepted.
    B. The singular `tag` alias used `get_string`, so `tag: ["a","b"]` was silently skipped. Now it goes through `list_param` like everything else.
    C. With the wider `slug_ends_at` rule, removing `#bug,` left an orphan space (`Fix , then ship`). `remove_tag` now absorbs the space in front of the marker when there is no trailing space to absorb.

    ## What did not work

    - My first `dispatch_update_task_tags_empty_array_clears_the_set` and `dispatch_update_task_assignees_empty_array_clears` tests PASSED before the fix — vacuously, because the seeding step they relied on was itself broken. Both were rewritten to seed through a working op and assert the pre-state. Read a green test twice before trusting it.
    - `attachments` cannot be asserted as the raw paths written. The entity layer copies each source into `.attachments/` and reads the field back as metadata objects; a path that does not exist on disk fails the write with `AttachmentSourceNotFound`. Tests use real temp files and assert the enriched names.
    - Assignee tests need `add actor` first. `assignees` is a reference field and dangling ids are pruned on write — that is finding ^n36mc1q, discovered this way.

    ## Verification

    - RED confirmed before each fix: 17 dispatch tests, 5 tag_parser tests, 4 round-2 tests. The `rewrite_body` post-condition and the served-tool tests were additionally proven by neutering the fix and watching them fail, then restoring.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'`: 7922 tests, 7921 passed. The single failure, `swissarmyhammer-tools::review_progress_notifications_test::review_working_emits_progress_notifications_per_pair_when_token_supplied`, is a load-sensitive flake: it spawns a real review agent, mentions neither kanban nor tags anywhere in its file, and passes alone and in two full package runs (1446/1446).
    - `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`: clean.
  timestamp: 2026-07-30T14:11:18.163832+00:00
- actor: claude-code
  id: 01kysq50jdxwng143we5gtcksk
  text: |-
    Implementation landed green in `doing`.

    Root cause was deeper than the card assumed: tags are not a stored field at all — they are `#tag` markers parsed out of the task body. Neither `AddTask` nor `UpdateTask` had a `tags` field, so serde skipped the param with no error. `tag task` and `untag task` each carried a private copy of the ULID-vs-slug resolution.

    Fix: new `task/tags.rs` owns resolution + body rewriting for all four ops, with `TagApply::{Append, Replace, Remove}` as the only difference. Shape tolerance moved to one `ref_list`/`list_param` pair in dispatch, shared by `tags`, `depends_on`, `assignees`, `attachments`.

    Sibling-field audit (step 4 of the card):
    - `depends_on` — already correct, was the model to copy.
    - `attachments` on `update task` — same defect, fixed. Field was declared on the struct (so the schema advertised it) but dispatch never read it.
    - `assignees` on both ops — partial defect, fixed. Scalar strings, stringified arrays and malformed values were dropped, and `assignees: []` could not clear.
    - `assignees` unresolvable actor ref — NOT fixed, filed as ^n36mc1q. `default_reference_validation` in swissarmyhammer-fields prunes dangling ids on write with no error, by policy, for every reference field. Changing that is a wider decision than this card. The MCP description now states the difference rather than implying assignees errors like tags.
    - `attachments` on `add task` — no defect.

    What did not work, for the next agent: the first cut passed my own tests but was still broken where they did not reach. `tag_parser::remove_tag` ended a match at a narrower boundary than `parse_tags` ends a slug, so `#bug,` parsed as a tag but could not be removed — which made `update task {tags: []}` a silent no-op, the exact class of bug this card exists to kill. `append_tag` also wrote markers onto fence and heading lines where the parser cannot see them. A second critic round then found three defects introduced by those fixes, including an attachment object shape that would wipe a task's attachment list while reporting success. All fixed RED-first.

    Verification (independent tester, not the implementer's claim):
    - `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` twice: 7922 tests, 7921 passed, 2 skipped.
    - 1363 kanban tests pass, 18 in the new `task::tags` module, plus 72 mcp::tools::kanban.
    - `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - The single failure (`review_progress_notifications_test`) is a pre-existing flake, already filed three times on 2026-07-21 as ^t681xdv, ^yh4m6ed, ^pxhzk1s — nine days before this card existed. Proven pre-existing: passes 8/8 in isolation, reproduces under CPU load alone with no full suite, and the test builds its own temp git repo so this diff cannot reach its input. No fourth card opened.
  timestamp: 2026-07-30T14:34:13.965544+00:00
- actor: claude-code
  id: 01kysweew5wa5fahhe9terjdbk
  text: |-
    Review round 5: all 19 findings satisfied, every box flipped to `- [x]`. Task stays in `doing`.

    ## What changed, per finding group

    **tag_parser.rs (6 findings) — fixed at the root, not at the cited lines.** The six findings collapsed to two root causes, so the whole file was restructured rather than patched line by line:
    - One markdown state machine, `markdown_lines()`, replaces the fence/heading logic that `parse_tags`, `remove_tag`, and `rename_tag` each carried. It yields `(line, tag_bearing)`; the reader and both writers now agree by construction.
    - One fence predicate, `is_fence_line()`. One heading predicate — `is_heading_line()` already existed but `parse_tags` inlined it; it now calls it (through `markdown_lines`).
    - One boundary rule, `tag_slug_at()`, returning the slug byte range. It replaces both the inline slug scan in `parse_tags` AND the `starts_with(pattern)` + `slug_ends_at` pair in the writers. `slug_ends_at`/`slug_starts_at` are gone, folded into it.
    - One inline-code skip, `skip_inline_code()`, replacing the repeated inner `while` loop. That loop was the 4th nesting level in all three functions; removing it is what actually fixes the three depth findings.
    - `remove_tag` and `rename_tag` are now two-line wrappers over one writer, `edit_tag_markers(text, slug, replacement: Option<&str>)` — `None` removes, `Some("#new")` renames. The data-driven form is shorter than either original.

    Nesting is now for/while(1) → if/match(2) → if(3), max, in every function.

    **task/tag.rs + task/untag.rs (3 findings).** `TagTask::execute` and `UntagTask::execute` were near-verbatim duplicates. Both are now 3-line wrappers over `tags::apply_one_tag_ref(ctx, task_id, tag_ref, mode)`. The only real difference — `tag task` runs `auto_create_body_tags`, `untag task` does not — is derived from the mode (`mode != TagApply::Remove`) rather than passed as a separate flag, so the two can never disagree. Doc comments added to both `pub fn new`.

    **task/tags.rs (1 finding).** Uppercase short id assertion added to `short_id_and_caret_forms_resolve`, plus the `^UPPERCASE` caret form.

    **task/update.rs (4 findings).** `with_attachments` now takes `impl Into<Value>`, matching its sibling builders. Three tests added: `test_update_task_with_attachments_round_trips`, `test_update_task_clear_scheduled_date`, `test_update_task_empty_tags_clears_every_tag`.

    **tools/kanban/mod.rs (5 findings).** `const KANBAN_INIT_PRIORITY: i32 = 55` replaces the literal. `swissarmyhammer_kanban::types::short_id(&tag_id)` replaces the hand-rolled `tag_id[len-7..]` slice. Two tests added/extended: `test_assignees_persist_across_input_shapes_via_served_tool` (all three wire shapes, add + update), and an add-task arm on `test_unresolvable_tag_ref_errors_via_served_tool`.

    ## Every new test was proven RED first

    None of these were taken on faith — each was watched failing against neutered production code, then restored:
    - assignees shape test + add-task unresolvable-tag test: neutered `explicit_assignees` back to array-only and `tag_refs` to `Ok(None)`. Both failed (`add task dropped assignees for shape=single string`).
    - attachments round trip: neutered the `attachments` arm of `UpdateTask::apply_to`. Failed with `got: []`.
    - the two tag-normalization regressions below: neutered each call site. Both failed.

    ## Regression found by the adversarial pass, and fixed

    The `double-check` agent ran a differential execution of the old vs new walkers — ~1M inputs across two alphabets — and found `parse_tags`/`remove_tag`/`rename_tag` equivalent for every slug the system can actually produce. It then found one real drift: `add tag` stores the name VERBATIM (no `normalize_slug`), and `delete tag` / `update tag` handed that raw name straight to the body writers. The old writers matched the literal (`starts_with("#v2.0")`); the new boundary rule does not, because `parse_tags` reads `#v2.0` as the tag `v2`.

    Fixed by normalizing at both call sites — which is what `cut tag`, `paste tag`, and `update tag`'s NEW name already do, so this is the prevailing pattern, not a new one. A body always carries `normalize_slug(tag_name)`; the writers must be handed that. This also repairs a latent bug that predates this card: a tag stored as `"Bug Fix"` could never have its markers renamed or stripped, because the body carries `#Bug-Fix` and the old code searched for `#Bug Fix`. Two regression tests added (`tag/delete.rs` gained a test module, which it had none of).

    ## What did not work, for the next agent

    1. **`mv file.bak file.rs` does not trigger a cargo rebuild.** `sed -i.bak` + `mv` back restores an OLDER mtime, so cargo's fingerprint check skips the crate and nextest silently runs the neutered binary. This cost a confusing cycle where restored-and-correct source kept "failing". Use `touch` after restoring, and distrust any test run that completes suspiciously fast.
    2. **`EntityContext` caches reads.** My first cut of both new tag tests captured `ectx` BEFORE the mutation to assert the seed state, then reused the same handle afterwards — and got the pre-mutation body back, so both tests failed against correct code. Re-acquire `ctx.entity_context()` after any mutation you intend to observe. The pre-existing `test_rename_tag_bulk_updates_task_descriptions` avoids this only by accident: it creates its context after the rename.
    3. Because of (2), the FIRST RED proof of those two tests was confounded — the tests were failing for their own bug, not the production one. The proof was redone after the fix, with a forced rebuild, and both genuinely go RED → GREEN on the production change alone.

    ## Verification

    - `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'`: 7929 tests, **7929 passed**, 2 skipped, 0 failed. The `review_progress_notifications_test` flake (^t681xdv, ^yh4m6ed, ^pxhzk1s) passed this round.
    - `cargo fmt --all -- --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - Nothing under `builtin/` was touched — those edits in the tree belong to a parallel session.
  timestamp: 2026-07-30T16:06:46.405738+00:00
position_column: doing
position_ordinal: '8280'
title: add task / update task silently discard the tags array
---
`add task` and `update task` both accept a `tags` array, return `ok: true`, and apply nothing. The array is discarded without an error or a warning.

A `tags` array must give the same result as calling `tag task` once per tag.

## Reproduction

```
add task { title: "...", column: "todo", tags: ["01KJZEPKJ35S76KF7E9HS5742J", "01KT7375T468PE35B87WY042DQ"] }
→ { "ok": ..., "tags": [] }          ← tag ids, dropped

update task { id: "^t7ebyn8", tags: ["bug", "init", "mirdan"] }
→ { "ok": true }
get task { id: "^t7ebyn8" }
→ { "tags": [] }                     ← tag names, dropped
```

The single-tag op works:

```
tag task { id: "^t7ebyn8", tag: "bug" }   → applied
```

So the caller must make N calls where the schema advertises one. `tag task` with a plural `tags` array fails loudly with `parse error: missing required field: tag`, which is correct behavior. The `add`/`update` path is the silent one.

## Why this matters

The response says `ok: true`. A caller has no way to know the tags were lost except by a follow-up `get task`. An agent that trusts the acknowledgement writes untagged cards and never learns. Silent input loss is worse than rejection.

Both forms were dropped, so the cause is not id-versus-name resolution:
- full ULIDs, on `add task`
- plain tag names, on `update task`

## Required change

1. Make `tags` on `add task` and `update task` apply, with the same resolution and the same create-if-absent behavior that `tag task` uses. Route both through one shared code path so the two can never disagree again.
2. Accept the forgiving shapes the board already accepts elsewhere: a single tag, a JSON array, or a stringified JSON array. Accept a full ULID, a short id, and a tag name.
3. An unresolvable tag must be an error, not a silent no-op — the same rule the board already states for `depends_on`.
4. Audit the sibling collection fields on `add task` and `update task` for the same defect — `assignees`, `depends_on`, `attachments`. Report what is affected. Do not assume `tags` is the only one.

## Acceptance

- `add task` with a `tags` array returns a task carrying those tags. Test must fail before the change.
- `update task` with a `tags` array replaces the tag set and `get task` confirms it.
- Equivalence test: a card built with one `add task { tags: [a, b, c] }` and a card built with three `tag task` calls end with the same tag set.
- An unknown/unresolvable tag ref returns an error, and the task is unchanged.
- Whatever the audit in step 4 finds gets a test too, or its own card. #bug #kanban

## Review Findings (2026-07-30 09:35)

- [x] `crates/swissarmyhammer-kanban/src/tag_parser.rs:16` — parse_tags has 5+ levels of nesting with nested loops (for loop containing while loop containing if containing another while loop), plus complex boolean conditions. This makes control flow hard to trace. Extract the inline code parsing logic into a separate function to reduce nesting. The backtick-skipping loop and tag-matching logic could each be their own functions.
- [x] `crates/swissarmyhammer-kanban/src/tag_parser.rs:23` — The heading line detection in `parse_tags` duplicates the logic in the `is_heading_line()` helper function. The helper was extracted later (line 76–80) but `parse_tags` inlines the condition while `remove_tag()` (line 109) and `rename_tag()` (line 160) call the function. This violates the one-code-path principle — the same pattern should be expressed once. Apply `is_heading_line()` in `parse_tags` at line 23. If the function cannot be called yet due to definition order, reorganize by moving the helper functions before `parse_tags`, or extract the condition to a shared named constant that all three functions use.
- [x] `crates/swissarmyhammer-kanban/src/tag_parser.rs:35` — The heading line detection in `parse_tags` at line 35 duplicates the logic in the `is_heading_line()` helper function (line 76–80). The same condition is inlined here instead of calling the extracted helper, violating the one-code-path principle. This is a separate violation from the fence marker check; both markdown patterns at line 35 level should be extracted or unified. Call `is_heading_line(line)` at line 35 in `parse_tags`, or move the helper function definition before `parse_tags` to allow the call. This unifies the three functions' heading detection into one code path.
- [x] `crates/swissarmyhammer-kanban/src/tag_parser.rs:124` — remove_tag has 4+ levels of nesting with nested loops (for loop containing while loop containing if containing another while loop). The inner while loop is at nesting depth 4, making the function hard to reason about. Extract the inline code skipping logic and tag removal logic into separate helper functions to reduce nesting depth and improve readability.
- [x] `crates/swissarmyhammer-kanban/src/tag_parser.rs:160` — The fence marker detection pattern `trimmed.starts_with("```") || trimmed.starts_with("~~~")` is repeated identically at lines 23, 109, and 160. This condition appears in three functions but is not factored into a shared helper, creating parallel arms that must be kept in sync. Extract the fence marker detection to a helper function `fn is_fence_line(trimmed: &str) -> bool { trimmed.starts_with("```") || trimmed.starts_with("~~~") }` and call it here and at lines 23 and 109.
- [x] `crates/swissarmyhammer-kanban/src/tag_parser.rs:178` — rename_tag has 4+ levels of nesting with nested loops (for loop containing while loop containing if containing another while loop). The structure mirrors remove_tag with same depth problem. Extract the inline code handling logic and tag replacement logic into separate functions. Consider creating a shared helper for the common 'skip inline code' pattern used in parse_tags, remove_tag, and rename_tag.
- [x] `crates/swissarmyhammer-kanban/src/task/tag.rs:30` — Public method `pub fn new` lacks a doc comment; struct-level documentation does not cover individual methods. Add a doc comment above the `new` method, e.g. `/// Create a new TagTask command`.
- [x] `crates/swissarmyhammer-kanban/src/task/tag.rs:39` — TagTask::execute and UntagTask::execute are near-verbatim duplicates that differ only by the TagApply mode parameter and a conditional call to auto_create_body_tags. Both follow identical structure: read entity, create refs slice, call apply_tag_refs with different mode, conditionally write, return thin ack. This should be extracted into a shared helper function. Extract a shared async fn that takes (ectx, entity_id, tag_ref, mode: TagApply, auto_create: bool) and handles the common logic, delegating to apply_tag_refs and optionally auto_create_body_tags. Both execute methods become 2-3 line wrappers that call this helper.
- [x] `crates/swissarmyhammer-kanban/src/task/tags.rs:423` — The new `tag_by_short_id()` function is case-insensitive (lowercases both needle and the calculated short_id before comparison), but the test `short_id_and_caret_forms_resolve` only exercises the canonical (lowercase) short id form returned by `short_id()`. If short ids ever come from case-mixed input sources, uppercase variants would not be validated. Add `assert_eq!(resolve_tag_ref(&tags, &short.to_uppercase()).unwrap(), "bug");` to the test to verify uppercase short ids resolve correctly.
- [x] `crates/swissarmyhammer-kanban/src/task/untag.rs:31` — Public method `pub fn new` lacks a doc comment; struct-level documentation does not cover individual methods. Add a doc comment above the `new` method, e.g. `/// Create a new UntagTask command`.
- [x] `crates/swissarmyhammer-kanban/src/task/update.rs:85` — Method `with_attachments` accepts concrete type `Value` instead of generic `impl Into<Value>`. This violates the API design rule to accept generics, not concrete types, and is inconsistent with other builder methods like `with_title` and `with_due` that use `impl Into`. Change signature to `pub fn with_attachments<V: Into<Value>>(mut self, attachments: V) -> Self` and store it as: `self.attachments = Some(attachments.into());`.
- [x] `crates/swissarmyhammer-kanban/src/task/update.rs:98` — The `with_attachments()` builder method (line 98) declares the capability to set attachments on an UpdateTask. However, no test verifies that attachments can be set via this method and read back. The prompt states the fix addresses attachments being declared but not read by dispatch, yet there is no test proving the round-trip works. Add a test that calls `with_attachments()` with sample attachment data (e.g. a file path string), executes the update, and verifies the attachments are present on retrieval via `fetch()`.
- [x] `crates/swissarmyhammer-kanban/src/task/update.rs:164` — The `clear_scheduled()` builder method (line 164) is defined but never tested, while the parallel method `clear_due()` (line 159) has an explicit test (`test_update_task_clear_due_date` at line 485). Both represent the same semantic operation on symmetric date fields; both should have equivalent test coverage. Add a test `test_update_task_clear_scheduled_date` that mirrors `test_update_task_clear_due_date`, calling `.clear_scheduled()`, executing, and asserting via `fetch()` that the scheduled field is null.
- [x] `crates/swissarmyhammer-kanban/src/task/update.rs:323` — The docstring for `tags` parameter (line ~246) explicitly documents that 'an empty list clears every tag', indicating this is a supported capability. However, no test verifies that calling `with_tags(vec![])`, executing the update, and reading back via `fetch()` produces a task with no tags. The diff adds multiple tests for non-empty tag sets but omits the empty-set round-trip. Add a test that calls `with_tags(vec![])` on a task that already has tags, executes the update, and asserts via `fetch()` that the tags array is empty.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:207` — Hardcoded priority value `55` should be a named constant. The comments reference `50` (Preamble) and `60` (Skills), suggesting this is part of a defined priority ordering system for initialization stages. Define a constant like `const KANBAN_INIT_PRIORITY: i32 = 55;` and use it instead of the literal.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:773` — Hardcoded length `7` for short ID extraction should be a named constant. This configures the length of the short ID derived from full IDs, but is hardcoded instead of using the `short_id()` function or a constant. Replace with `let short = swissarmyhammer_kanban::types::short_id(&tag_id).to_lowercase();` to reuse the existing helper, or define `const SHORT_ID_LEN: usize = 7;` and use `tag_id[tag_id.len() - SHORT_ID_LEN..]`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:1186` — Manual short ID extraction instead of calling shared utility. The test manually slices and lowercases the tag ID to extract a 7-character short form, duplicating logic that already exists in `swissarmyhammer_kanban::types::short_id()` used in the parallel `test_depends_on_persists_across_input_shapes_via_served_tool()` test above. Replace `let short: String = tag_id[tag_id.len() - 7..].to_lowercase();` with `let short = swissarmyhammer_kanban::types::short_id(&tag_id);` to reuse the existing utility function and keep short ID extraction logic in one place.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:1289` — The diff implements shape-tolerance handling (single string, JSON array, stringified array) for tags, depends_on, assignees, and attachments via a shared dispatch function. The test `test_tags_persist_across_input_shapes_via_served_tool` comprehensively proves tags work with all three shapes in both add and update operations. However, no corresponding shape-tolerance test is added for assignees, even though the prompt states the fix applies the same treatment: 'assignees on both ops — scalar strings, stringified arrays and malformed values were dropped.' If assignees inherit the same dispatch handling as tags, the same test coverage should exist. Add a test `test_add_task_assignees_persist_across_input_shapes_via_served_tool` that mirrors the tags test, verifying that assignees work with single-string, JSON-array, and stringified-array shapes in both add and update operations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:1314` — The test `test_unresolvable_tag_ref_errors_via_served_tool` verifies that update task errors on an unresolvable tag ID (line 1322: `json!("01KJZEPKJ35S76KF7E9HS5742J")`), but does not test add task with an unresolvable tag ID. Per the docstring ('An id reference that names no tag is an error'), both operations should enforce the same error behavior. If the fix unifies error handling across add and update, both should have equivalent test coverage. Add an assertion that verifies add task also errors when given an unresolvable tag ID, mirroring the update task behavior tested at line 1322.