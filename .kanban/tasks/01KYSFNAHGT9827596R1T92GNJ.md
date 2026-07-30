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
- actor: claude-code
  id: 01kyt0775hk4ktpz9frq5qz486
  text: |-
    Review round 6: all 9 findings satisfied, every box flipped to `- [x]`. Task stays in `doing`.

    The 9 findings were 4 distinct defects — the engine reported three of them twice from different lines.

    ## What changed

    **One shared walker (2 findings, the important one).** New `crates/swissarmyhammer-kanban/src/tag/shared.rs` holds `apply_tag_edit_to_all_tasks(ectx, edit_fn)`. `delete.rs` and `update.rs` each carried their own copy of the "list every task, rewrite its body through `tag_parser`, write back on change" loop, differing only in the parser call. Both are now one-line calls: `remove_tag` and `rename_tag` respectively. The iteration-2 collapse inside `tag_parser.rs` gave one reader and one writer; this removes the second duplicated walker that was sitting above it.

    `tag/shared.rs` mirrors the existing `task/shared.rs`: private `mod shared;`, `pub(crate)` items. No new dependency edge — `tag` already used `tag_parser` and `EntityContext`. The walk stays in the same position in both ops (after the duplicate-name check and before `entity.set("tag_name", ...)` in update; after the read and before `ectx.delete` in delete), so the entity layer sees the identical read/write sequence.

    I swept the crate for a third copy. There is none. `tag/cut.rs` and `tag/paste.rs` edit ONE task. `task/tags.rs::strip_all_tags` folds over one body. `actor/delete.rs` has the same shape but edits `assignees`, not tag markers — a different job, not this card.

    **Missing doc comments (5 findings, 3 real).** `DeleteTag::new`, `UpdateTag::with_color`, `UpdateTag::with_description`. I also documented `UpdateTag::new` and `with_name` — same class, same file, and the validator would have filed them next round.

    **Test scope (1 finding).** The `test_tags_persist_across_input_shapes_via_served_tool` docstring claimed "every ref format" but the `ref_form` loop had no uppercase entries. Took the preferred option: made the docstring true. The loop now spells out both letter cases of both ID forms — `tag_id.to_uppercase()`/`to_lowercase()`, `short.to_uppercase()`/`short`, `^SHORT`/`^short`. The id-as-issued form was dropped in favor of the explicit case pair so the coverage does not depend on which case the ULID generator emits.

    ## RED proofs, one of which I had to redo

    - **Uppercase short id at the MCP boundary.** My FIRST proof was unsound and the `double-check` agent caught it. I had neutered both `tag_by_id` and `tag_by_short_id`, so the loop aborted at index 1 on the full uppercase ULID — an entry that is byte-identical to the `tag_id.clone()` it replaced, because `ulid::Ulid::new().to_string()` already emits uppercase. The run never reached the genuinely new cases. Redone with the neutering aimed ONLY at `tag_by_short_id`'s `needle.to_lowercase()`: the failure now lands where it should, `add task dropped tags for ref_form=FS0FWF4`, and shows the exact silent-loss mode this card exists to kill — a tag literally named `FS0FWF4` gets created instead of resolving.
    - **The shared walker's blast radius.** New test `edit_rewrites_only_the_tasks_carrying_the_marker` in `tag/shared.rs`: a bystander task must come out byte-identical while the marked one is rewritten. Proven RED by swapping `remove_tag` for an over-matching `edit_fn` — the bystander came back mangled (`No MANGLED here`).

    ## What did not work, for the next agent

    1. **The write-skip guard is not testable, and my first test for it was worthless.** The `double-check` agent asked for a test on `if new_body != body`, reasoning that each `ectx.write` appends a changelog line. I wrote it — and it PASSED with the guard deleted. Probed further: with the guard gone, an unchanged write leaves the task's `.md` and `.jsonl` byte-identical AND its mtime untouched. **The entity layer already discards a write that changes no field.** The guard is a call-avoidance optimization with zero observable effect. I reworded the doc to say exactly that instead of asserting a persistence contract no test can hold, and replaced the test with the bystander test above, which holds something real.
    2. **`kanban-cli` is noun-first, and the wrong order fails SILENTLY.** `kanban update task --id ... --description ...` printed nothing, exited 0, and wrote nothing. The correct form is `kanban task update --id ...`. I burned a cycle believing a caching problem before checking `--help`. (Note the irony: a silent no-op on a card about silent no-ops.)
    3. The `mv file.bak file.rs` trap from round 5 held. I used `cp` + `touch` on every restore.

    ## Verification

    - `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'`: 7930 tests, **7930 passed**, 2 skipped, 0 failed. The `review_progress_notifications_test` flake (^t681xdv, ^yh4m6ed, ^pxhzk1s) passed this round.
    - `cargo fmt --all -- --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - Files changed: `tag/shared.rs` (new), `tag/delete.rs`, `tag/update.rs`, `tag/mod.rs`, `tools/src/mcp/tools/kanban/mod.rs`.
    - Nothing under `builtin/` was touched. The stray `.kanban` under `crates/swissarmyhammer-mcp-proxy/` dates from Jul 5 and is not from this session.
  timestamp: 2026-07-30T17:12:43.441805+00:00
- actor: claude-code
  id: 01kyt366fayba54nfwa4d5ewdv
  text: |-
    Review round 7: all 10 findings satisfied, every box flipped to `- [x]` (38/38). Task stays in `doing`.

    All 10 findings were in ONE file, `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` — pre-existing defects in the enclosing file, surfaced because this card touched it. The new `tag/shared.rs` walker came back clean.

    ## Error message casing (6 findings)

    The rule is `builtin/validators/rust/rules/error-handling.md`: "Display messages on errors: lowercase, no trailing punctuation." Swept the WHOLE file, not only the five cited lines:

    - `"Cannot determine current directory"` -> `"cannot determine current directory"` (was duplicated in `init` AND `deinit`; now one copy)
    - `"No .kanban directory found"` -> lowercase (same, was duplicated)
    - `"Failed to register merge drivers: {e}"` -> `"failed to register merge drivers: {e}"`
    - `"Failed to remove merge drivers: {e}"` -> `"failed to remove merge drivers: {e}"`
    - `"Failed to parse kanban operation: {}"` -> lowercase
    - `tracing::warn!("Failed to list tasks for plan")` -> lowercase (not cited, same class)

    Left alone on purpose: `InitResult::ok` messages ("Kanban merge drivers registered", "Kanban tool initialized"). Those are SUCCESS messages, not error Display messages. The engine did not flag them, and the sibling shell tool spells its `ok` messages the same way (`"Shell tool deinitialized"`), so lowercasing them would break the prevailing pattern to satisfy a rule that does not cover them.

    ## Structure (4 findings)

    **`Debug` derive.** `#[derive(Default)]` -> `#[derive(Debug, Default)]`. `mirdan::mcp_config::McpServerEntry` already derives `Debug`, so the field carries.

    **`schema()` / `schema_full()`.** Both read `kanban_operations()` and hand it to a generator. Collapsed onto `build_schema(generate: fn(&[&dyn Operation]) -> Value)`; each method is now one line naming its generator. One roster read, so the compact and full schemas can never disagree about which operations exist.

    **The two duplicated `init`/`deinit` blocks (2 findings) — collapsed together, not separately.** They are the same two steps pointing opposite ways, so patching each block with its own helper would have left two parallel skeletons. Instead:

    - `LifecycleSpec` — a const table holding EVERY value that differs: the applier fn pointer, the error/verb/message/ok strings, and the fallback message. `INIT_SPEC` and `DEINIT_SPEC` are the only two instances.
    - `KanbanTool::run_lifecycle(&spec, scope, reporter)` — the single skeleton. `init` and `deinit` are one-line delegations.
    - `merge_driver_result(spec, name, reporter)` — the merge-driver step, once.
    - `unregister_mcp_server_entry` — a 3-line adapter giving `unregister_mcp_server` the `register_mcp_server` signature (removal needs only the name), so both directions fit one `McpServerApplier` field.

    The two directions were NOT symmetric, and the asymmetry is now explicit data rather than buried in duplicated code: `abort_on_mcp_error: true` for init (stop, so no half-configured agent is left behind), `false` for deinit (carry on, so teardown strips as much as it can reach). That one flag is the whole behavioral difference.

    Two old early `return results` statements in the merge-driver blocks were dropped as cosmetic: nothing followed them except `if results.is_empty()`, which cannot fire once a result has been pushed. Verified by enumeration, not by assumption.

    **`is_task_modifying_operation`.** 12 `(verb, noun)` arms in a `matches!` where every arm returned `true` — data written as control flow. Now `const TASK_MODIFYING_OPERATIONS: &[(Verb, Noun)]` + `.contains(&(verb, noun))`. `Verb`/`Noun` already derive `Copy + PartialEq`, so no new bounds. Proved the pair set is byte-identical by extracting both lists with `grep -oE 'Verb::[A-Za-z]+, Noun::[A-Za-z]+' | sort` from `git show HEAD:` and from the new file, then `diff` — empty.

    ## RED proof

    New test `test_tool_lifecycle_no_board_skips_merge_drivers`. Project scope with no `.kanban/` board: BOTH directions must report exactly one SKIPPED result reading `no .kanban directory found`. It pins three things at once — the lowercase wording, the skip-not-error contract, and the fact that init and deinit now reach the same extracted helper.

    Proven RED by reverting only that one string to `"No .kanban directory found"`: failed with `left: "No .kanban directory found" / right: "no .kanban directory found"` on the `init` arm. Restored with `cp` + `touch` (the round-5 mtime trap), re-ran, GREEN. Because the neutered string was a SINGLE occurrence and both loop arms consumed it, the RED also proves both directions share one code path — the point of the extraction.

    The merge-driver skip path had no test before this round.

    ## Verification

    - `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'`: 7931 tests, **7931 passed**, 2 skipped, 0 failed. The `review_progress_notifications_test` flake (^t681xdv, ^yh4m6ed, ^pxhzk1s) passed this round.
    - `cargo nextest run -p swissarmyhammer-tools -E 'test(mcp::tools::kanban)'`: 74/74.
    - `cargo fmt --all -- --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - Files changed: `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` only (+222 / -132). Nothing under `builtin/` — confirmed with `git status --porcelain | grep builtin`, zero hits. No stray `.kanban` dirs created; the one under `crates/swissarmyhammer-mcp-proxy/` predates this session, as round 6 recorded.

    ## For the next agent

    The traps from rounds 5 and 6 all held and cost nothing this round because they were written down: `cp` + `touch` on every restore (never `mv` a `.bak`, it restores an older mtime and cargo skips the rebuild), and `kanban task update` is noun-first (`kanban <noun> <verb>`); the wrong order exits 0 and writes nothing.

    New note: the `files` MCP edit op echoes the WHOLE post-edit file back as `tagged_content`. On a 3000-line file that overflows the tool-result budget and the edit still lands, which reads like a failure. For large files, patch with a `python3` script through `shell` and verify with `grep`/`sed`.
  timestamp: 2026-07-30T18:04:35.690803+00:00
- actor: claude-code
  id: 01kyt3y0jre3fm3ghz06gq4ayv
  text: |-
    Adversarial pass (`double-check`) returned REVISE with 2 findings. Both acted on. Re-verified green.

    ## What the critic confirmed

    It reconstructed the pre-change `init`/`deinit` from `git show HEAD:` and enumerated every input state — `mcp_server` Some/None x applier ok/error x scope User/Local/Project x board present/absent x merge-driver ok/error. Result count, order, status, message, and reporter emit order agree in every state. No `LifecycleSpec` field is crossed between the two specs. The 12 `(verb, noun)` pairs, the `schema`/`schema_full` routing, and the `unregister_mcp_server_entry` adapter all check out.

    ## Finding 1 — the asymmetry flag had no test (accepted, fixed)

    Fair hit, and the sharpest one available: the refactor converts a control-flow difference into a single `bool` in a const table, and every existing lifecycle test takes the MCP success path. Flip either flag — or copy one spec from the other — and the suite stays green while a half-configured agent gets left behind. The flag was the only unguarded part of the change.

    Making the applier fail is not obvious. Reading `mirdan::install::for_each_agent_strategy`: a PER-AGENT failure only emits an `InitEvent::Warning` and still returns `InitResult::ok`. The one path that yields an error result is `detected_agents_or_error()`, i.e. `load_agents_config()` failing. So the deterministic lever is an UNPARSEABLE `MIRDAN_AGENTS_CONFIG`, not an unwritable MCP target.

    New test `test_mcp_applier_error_aborts_init_but_not_deinit`. Points `MIRDAN_AGENTS_CONFIG` at `agents:\n  - id: [unclosed`, with a `.kanban/` board present so the merge-driver step has a result to contribute whenever it is reached:
    - `init` at Project scope must return exactly 1 result, status `Error` — it abandoned the merge-driver step.
    - `deinit` at Project scope must return exactly 2 — the error, then a non-error merge-driver result — it carried on.

    That is precisely the old behavior: old `init` did `return vec![InitResult::error(...)]`, old `deinit` pushed the error and fell through. The test encodes the preserved asymmetry, it does not invent one.

    Both flags proven RED independently:
    - `INIT_SPEC.abort_on_mcp_error: true -> false`: init returned 2 results, `left: 2 / right: 1`.
    - `DEINIT_SPEC.abort_on_mcp_error: false -> true`: deinit returned 1, `left: 1 / right: 2`.
    Restored with `cp` + `touch` each time; GREEN after each restore.

    ## Finding 2 — the same message, capitalized, in the sibling entry point (accepted, fixed)

    `apps/kanban-cli/src/commands/serve.rs` carries a second copy of `"Failed to parse kanban operation: {e}"` — the same string, for the same tool, in the other MCP call handler. Lowercasing only `mod.rs` would have made `sah serve` and `kanban serve` disagree about the same failure. Fixed. It was the only capitalized error message in that file.

    Not touched, and deliberately: `mirdan::install::detected_agents_or_error` builds `"Failed to load agents config: {e}"`. It is a different crate, not cited by any finding, and the shell tool carries similar pre-existing capitals. Sweeping mirdan belongs on its own card, not this one.

    ## Re-verification after the revisions

    - `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'`: 7932 tests, **7932 passed**, 2 skipped, 0 failed. The `review_progress_notifications_test` flake (^t681xdv, ^yh4m6ed, ^pxhzk1s) passed again.
    - `cargo nextest run -p swissarmyhammer-tools -p kanban-cli` over the kanban, lifecycle and serve tests: 199/199.
    - `cargo fmt --all -- --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: exit 0, zero warnings.
    - Files changed: `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` (+275/-132) and `apps/kanban-cli/src/commands/serve.rs` (1 line). Nothing under `builtin/`.

    Task stays in `doing`, 38/38.
  timestamp: 2026-07-30T18:17:36.088565+00:00
- actor: claude-code
  id: 01kyt8fnws8qmas5ae5331r2jj
  text: |-
    Review round 8: all 18 findings satisfied. Every box is `- [x]` (56/56). Task stays in `doing`.

    The 18 findings were 3 debt classes. I cleared each class fully in the two files, not only at the cited lines.

    ## Class 1 — missing doc comments (15 cited, 10 real)

    The engine reported the same item twice from adjacent line numbers. The 10 real items were all trait-impl methods in `tools/kanban/mod.rs`: `name`, `display_name`, `category`, `priority` on the `Initializable` impl, and `name`, `description`, `schema`, `schema_full`, `operations`, `execute` on the `McpTool` impl. `McpTool::name` was NOT cited. I documented it too.

    `apps/kanban-cli/src/commands/serve.rs` needed nothing outside its test module. Its two undocumented test functions got docs anyway, so that file is now at 100 percent.

    **Exhaustiveness proof.** I wrote a checker (`scratchpad/check_docs.py`) that finds every `fn`/`struct`/`enum`/`trait`/`const`/`type` outside `#[cfg(test)]`, walks up past attributes, and demands a `///` above it. Against `git show HEAD:` it reports exactly the 10 items above, and 0 for serve.rs. Against the new tree it reports 0 for both files. The checker is therefore proven to detect this class, not just to agree with me.

    **Stronger proof: rustdoc is silent too.** `cargo doc --no-deps` now reports ZERO warnings for both files. That repaired two broken links that predate this round: `[`Self::init`]` did not resolve (the trait method needed its full path), and `[`EntityError`]` in serve.rs named no item in scope. Same debt class, same files.

    Not swept, on purpose: `mod.rs` still has 59 undocumented test functions. `builtin/validators/missing-docs/rules/missing-docs.md` exempts `#[test]`/`#[tokio::test]` items and `mod tests` explicitly, and no round has ever cited one. serve.rs is now at 100 percent only because it had just two.

    ## Class 2 — error message casing (2 cited)

    `"Unknown tool: {}"` in serve.rs lowercased, plus the test assertion that pinned the old spelling.

    **Exhaustiveness proof.** `grep -noE '"[A-Z][^"]{3,}'` over the whole file now returns 8 hits, none of them an error message: the tool description, two `env!("CARGO_PKG_VERSION")`, four test board names, and one test assertion string. Zero capitalized error Display messages remain.

    Left capitalized on purpose, as already adjudicated: `InitResult::ok` success messages.

    The same capitalized string lives in `swissarmyhammer-tools/src/mcp/server.rs`, `agent-client-protocol-extras/src/test_mcp_server.rs` and `claude-agent/src/tools.rs`. Those are outside the two files this card touches, so a review of this diff cannot cite them. Filed as ^p4mp9n6 rather than expanding scope.

    ## Class 3 — batch response shape (1 cited)

    Extracted the inline wrapper out of `execute` into a pure helper, `attach_plan(response, plan) -> Value`. An object response takes `_plan` as a sibling key. Anything else nests under `result`. The branch was untestable inside a 90-line async method; as a pure function it takes two direct unit tests.

    The behavior predates this card (commit 20e4a9c55, February), but the card owns it now, so the test lands here.

    **RED proved on both branches, one at a time:**
    - Batch branch made to return the array and drop the plan: `test_attach_plan_wraps_batch_array_response` failed with `got: [{"id":"01ABC",...}]`.
    - Object branch made to skip the insert: `test_attach_plan_merges_into_object_response` failed with `left: Null`.
    - The casing change was proved too. Reverting the one string to `"Unknown tool: "` failed the existing test with `got: Unknown tool: not-kanban`.

    ## What the adversarial passes found — 7 findings, all doc accuracy, all fixed

    Two `double-check` runs. The first proved `attach_plan` byte-identical to the old inline code for every input class, then returned REVISE on 4 doc claims. The second returned REVISE on 3 more. Every one was true. I checked each against the source myself before changing anything.

    1. `schema()` claimed the wire schema carries "per-op required fields". It does not. `generate_mcp_schema_wire` emits `properties` with only `op` and `required: ["op"]`. The per-op map is `x-op-signatures`, which is member 5 of `WIRE_DROPPED_KEYS` — and an existing test asserts every one of those keys is absent from the wire schema. My doc contradicted a green test.
    2. `schema()` then claimed the description "has to name each op's arguments". The check exists (`required_params_missing_from_description`) but has only two callers, neither of them kanban — and `description.md` names no required parameter at all, so kanban would fail it today. Replaced the obligation with the fact.
    3. `schema_full()` claimed "the wire surface plus the five entries". It is not a superset: the full schema has NO top-level `required`, and its `properties` is the flat union of every op's parameters, not `op` alone.
    4. `priority()` claimed kanban runs "after the preamble and before skill deployment". Neither half holds. The registry holds two components — `ProjectStructure` (40) and this tool — and nothing declares 50 or 60. Skills, agents, preamble and statusline are `Profile` fields that `init_profile` handles BEFORE the registry runs. I had copied the stale comment on `KANBAN_INIT_PRIORITY` into a second place; both are now corrected together, so they cannot drift.
    5. A follow-up clause, "the pipeline is spaced in 10s", was also unsupported: the four priorities in the workspace are 0, 22, 40, 55. Deleted.
    6. `execute()` claimed one operation yields "a lone object". `next task` returns `Value::Null` on an empty board, and `execute` passes a single result through verbatim.
    7. `attach_plan`'s unreachability argument named only one precondition. It needs two: the arguments-object rule AND the fact that all twelve `TASK_MODIFYING_OPERATIONS` return JSON objects. The critic checked all twelve; they do. Neither precondition is enforced, and the doc now says so.

    ## For the next agent

    **A doc comment is a claim. Verify it like code.** Seven of my ten new docs were wrong on the first or second attempt — not vague, wrong, and one of them contradicted a test that was passing in the same run. Compilers and clippy do not read prose. The only cheap defence is to open the function you are describing before you describe it.

    **`cargo doc --no-deps` is a free extra gate.** It caught two broken intra-doc links that predate this card and that clippy never mentions. Run it on any file you add docs to; grep its output for that file's path.

    The traps recorded in rounds 5 to 7 all held and cost nothing:
    - `cp` + `touch` on every restore. Never `mv` a `.bak` back — that restores an older mtime, cargo skips the rebuild, and nextest silently runs the neutered binary.
    - The `files` MCP edit op echoes the WHOLE post-edit file. On a 3000-line file that overflows the tool budget while the edit still lands. I used a `python3` script through `shell` for every large patch.
    - `kanban` CLI is noun-first (`kanban task update`). I used it to flip the 18 boxes, passing the new body as `--description "$(cat file)"`, so the change went through real dispatch and kept the changelog honest.

    New note: the running MCP server is an OLD binary. `add task {tags: ["bug"]}` on ^p4mp9n6 still dropped the tags, because THIS card's fix is uncommitted and not yet in the server the agent talks to. That is not a regression. Use `tag task` until the fix ships.

    ## Verification

    - `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'`: 7934 tests, **7934 passed**, 2 skipped, 0 failed. Run three times this round, green each time — before the doc revisions, after the first set, and after the second. The `review_progress_notifications_test` flake (^t681xdv, ^yh4m6ed, ^pxhzk1s) passed all three.
    - `cargo fmt --all -- --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - `cargo doc --no-deps`: zero warnings attributable to either file.
    - Files changed: `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` and `apps/kanban-cli/src/commands/serve.rs`. Nothing else. `git status --porcelain | grep builtin` gives zero hits. No new stray `.kanban` directory; the one under `crates/swissarmyhammer-mcp-proxy/` predates this session, as round 6 recorded.
  timestamp: 2026-07-30T19:37:09.273802+00:00
- actor: claude-code
  id: 01kytcr086qc5k52ypgqercxnk
  text: |-
    Review round 9: the single remaining finding is satisfied. 57/57 boxes are `- [x]`. Task stays in `doing`.

    ## Answer to the finding: all 12 operations DO carry a correct affected_task_id

    No operation was broken. All 8 that lacked a read-back test pass one now. The write side was already right; only the proof was missing.

    The reason is structural, and worth recording: every one of the 12 returns `task_helpers::task_mutation_ack(&entity)`, which emits `{ok, id, short_id}` where `id` is the TASK entity id. `add task` returns `slim_task_json`, which also carries `id`. `execute` copies `result["id"]` into `_plan._meta.affected_task_id`. One ack helper, one extraction, so the 12 cannot disagree.

    The two comment ops were the ones worth doubting: `update comment` and `delete comment` both ack the OWNING task, never the comment member, and their tests assert exactly that with an `assert_ne!(comment_id, task_id)` guard so the assertion cannot pass by the two ids coinciding.

    ## Shape of the change

    Rather than 8 more copies of a 25-line skeleton — which is the debt class rounds 5 to 8 kept filing — the 4 existing tests and the 8 new ones now share one section and one set of helpers:

    - `run_op(tool, context, json!({...}))` — build args, execute, parse. The old `get_task` helper now delegates to it.
    - `plan_probe_board(temp, title)` — board plus one seeded task; returns the whole `add task` response, because `add task` is itself under test.
    - `assert_plan_affects(data, task_id, op)` — the one assertion, naming the op in its failure message.
    - `add_probe_actor`, `add_probe_comment` — the two non-trivial setups.

    Each test is now 8 to 15 lines and nextest still reports one result per operation. No assertion from the 4 pre-existing tests was dropped; the adversarial pass verified that against `git show HEAD:` independently.

    ## RED proofs

    **Neuter 1 — drop the id extraction in `execute`.** All 12 failed. The failure output matters: `_plan` was still attached, only `affected_task_id` was `Null`. That is the exact proof the finding asked for — these are read-backs of the id, not "a `_plan` key exists" checks.

    **Neuter 2 — remove the 8 newly-covered pairs from `TASK_MODIFYING_OPERATIONS`.** Exactly those 8 failed; the 4 pre-existing tests passed. This proves each new test is pinned to its OWN operation and is not riding on a sibling's plan — the real risk for `untag` (which tags first), `unassign` (which assigns first) and the two comment ops (which add a comment first).

    **Neuter 3 — substitute one pair, keeping the length at 12.** The coverage guard failed. See below.

    ## A real bug found, filed as ^qc0jkf8, NOT fixed here

    `_plan.entries` is ALWAYS empty. `build_plan_data` calls `ListTasks::new().execute(ctx)` and then `tasks.as_array()`, but `ListTasks::execute` returns an OBJECT `{"tasks": [...], "count": N}`. `as_array()` gives `None`, `unwrap_or(&Vec::new())` supplies an empty list, and the entry-building `.map()` never runs. Silent — no error, no warning.

    Live evidence, from the `move task` that started this session on a board holding hundreds of cards: `"_plan":{"_meta":{"affected_task_id":"01KYSF...","trigger":"move task"},"entries":[]}`. The card filed for it demonstrates the same thing in its own `add task` response.

    So the module header quotes the ACP rule "Complete plan lists must be resent with each update" while the tool resends an empty list every time. `task_to_plan_entry` and the status/priority mapping beside it are dead code today.

    Deliberately not fixed on this card: it is a production behavior change reaching every `_plan` consumer, on a test-only card at its final review round. Same precedent as ^n36mc1q, ^tnr56gg and ^p4mp9n6.

    ## Adversarial pass: REVISE with 6 findings, 5 accepted, 1 rejected on evidence

    1. **`_plan.entries` always empty** — accepted, filed as above. Also deleted the doc clause I had written that said the affected id "cannot come from the task list `_plan` enumerates". `_plan` enumerates nothing, ever, so the clause was false.
    2. **The coverage guard overclaimed** — accepted. My first version pinned only `len() == 12`, while its doc claimed the list and its proof "cannot drift apart". Substituting one pair keeps the length at 12 and the guard stayed green. It now pins the 12 PAIRS. Proven by substituting `(Complete, Task)` for `(Archive, Task)`: the guard fails and prints both lists. The duplicated pair list is the mechanism of a drift guard, not accidental duplication, and the doc now says so.
    3. **`test_add_task_plan_carries_affected_task_id` was circular** — accepted, and the sharpest catch. It read `task_id` from `added["id"]` and compared it to `_plan._meta.affected_task_id`, which production fills FROM that same field. It could never disagree. Now it takes the id OUT of the plan, calls `get task` with it, and asserts the stored title. Re-proved RED.
    4. **`run_op` doc said "the real wire response"** — accepted. The served path in `mcp/server.rs` runs `fold_in_diagnostics` after `execute`, so `run_op` stops one layer short. Doc corrected and the difference stated.
    5. **`add comment` doc overgeneralized** — accepted. Dropping the `(Add, Comment)` arm skips only `add comment`; the other two comment ops have their own arms. My own neuter 2 had already shown this.
    6. **"14 spaces baked into the guard's panic message"** — I first REJECTED this with a standalone `rustc` program proving Rust's `\`-newline continuation strips leading whitespace. I was wrong, and the way I was wrong is the lesson: `cargo fmt` had already COLLAPSED the continuation into a single-line literal on disk, materialising the 14 spaces. I had verified the source form I wrote, not the form on disk. Confirmed by reading the file, then fixed with a short one-line message.

    ## For the next agent

    **Verify against the file on disk, not against what you wrote.** A formatter runs between the two. My rustc experiment was correct about Rust and still gave the wrong answer, because it tested a string literal that no longer existed in that form. `grep` the actual line before concluding.

    The traps from rounds 5 to 8 all held: `cp` plus `touch` on every restore, never `mv` a `.bak`; the `files` MCP edit op echoes the whole 3400-line file and overflows the tool budget while the edit still lands, so patch large files with `python3` through `shell` and verify with `grep`; `kanban` CLI is noun-first (`kanban task update`).

    ## Verification

    - `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'`: 7943 tests, **7943 passed**, 2 skipped, 0 failed. The `review_progress_notifications_test` flake (^t681xdv, ^yh4m6ed, ^pxhzk1s) failed on an earlier run this round and passed alone in 13 s, then passed in the final full run.
    - `cargo nextest run -p swissarmyhammer-tools -E 'test(mcp::tools::kanban)'`: 86/86, up from 74.
    - `cargo fmt --all -- --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo doc --no-deps`: zero warnings attributable to the file.
    - Files changed: `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` only. Nothing under `builtin/` — `git status --porcelain | grep builtin` gives zero hits, and the `builtin/` edits present at session start belong to a parallel session. No new stray `.kanban` directory.
  timestamp: 2026-07-30T20:51:36.326749+00:00
- actor: claude-code
  id: 01kytevy0mdqcg23svce23gjn6
  text: |-
    Review finding (2026-07-30 16:06) fixed: run_lifecycle nesting flattened.

    Change — `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`:
    - Moved the MCP-server step out of `run_lifecycle` into a new private helper `KanbanTool::mcp_server_results`. The helper uses a let-else for "no server entry injected" and a two-arm match that maps an applier error to a single `InitResult::error`, or passes the applier results through. Max nesting depth in the helper: 2 (match arms).
    - `run_lifecycle` now starts from the helper's results and makes ONE flat decision: `if spec.abort_on_mcp_error && applier_error(&results).is_some() { return results; }`. Max nesting depth in `run_lifecycle`: 1 (the top-level ifs). The reported chain if let -> match -> Some arm -> if is gone.
    - Behavior is unchanged: an applier error still contributes exactly one error result, init still abandons the merge-driver step, deinit still carries on to it.

    Mutation proof that `abort_on_mcp_error` still controls the behavior (both directions, after the refactor), with `test_mcp_applier_error_aborts_init_but_not_deinit`:
    - INIT_SPEC true -> false: FAIL. "init must abort on an MCP applier error" left: 2, right: 1 (the merge-driver result appeared).
    - DEINIT_SPEC false -> true: FAIL. "deinit must carry on to the merge-driver step" left: 1, right: 2 (the merge-driver result disappeared).
    - Both flags restored: PASS.

    Verification: `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` 7943 passed, 0 failed, 2 skipped. `cargo fmt` clean (no diff). `cargo clippy --workspace --all-targets -- -D warnings` clean. Card is 58/58. Left in doing, not committed.
  timestamp: 2026-07-30T21:28:42.260217+00:00
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

## Review Findings (2026-07-30 11:23)

- [x] `crates/swissarmyhammer-kanban/src/tag/delete.rs:24` — Public constructor `new` lacks documentation comment. Builder methods must document their purpose. Add a doc comment above the function, e.g., `/// Create a new DeleteTag command for the given tag ID.`.
- [x] `crates/swissarmyhammer-kanban/src/tag/delete.rs:25` — Public method `new()` lacks a doc comment. All public items must be documented to explain their purpose and usage. Add a doc comment above the method explaining what it does, e.g.: `/// Create a new DeleteTag command for the given tag ID.`.
- [x] `crates/swissarmyhammer-kanban/src/tag/delete.rs:36` — The pattern of iterating over all tasks and applying a tag_parser operation to their bodies is duplicated between delete.rs and update.rs. The identical loop structure differs only in the tag_parser function called (remove_tag vs rename_tag). Extract a shared helper function `apply_tag_edit_to_all_tasks(ectx: &EntityContext, edit_fn: impl Fn(&str) -> String) -> Result<()>` in task/shared.rs or a new tag/shared.rs. Call it from both delete.rs as `apply_tag_edit_to_all_tasks(&ectx, |body| tag_parser::remove_tag(body, &slug))` and from update.rs as `apply_tag_edit_to_all_tasks(&ectx, |body| tag_parser::rename_tag(body, &old_slug, &normalized))`.
- [x] `crates/swissarmyhammer-kanban/src/tag/update.rs:49` — Public builder method `with_color` lacks documentation. Builder methods must document what they set. Add a doc comment, e.g., `/// Set the tag color (6-character hex without #).`.
- [x] `crates/swissarmyhammer-kanban/src/tag/update.rs:54` — Public builder method `with_description` lacks documentation. Builder methods must document what they set. Add a doc comment, e.g., `/// Set the tag description.`.
- [x] `crates/swissarmyhammer-kanban/src/tag/update.rs:60` — Public builder method `with_color()` lacks a doc comment. Add a doc comment: `/// Set the tag's color (6-character hex without #).`.
- [x] `crates/swissarmyhammer-kanban/src/tag/update.rs:64` — Public builder method `with_description()` lacks a doc comment. Add a doc comment: `/// Set the tag's description.`.
- [x] `crates/swissarmyhammer-kanban/src/tag/update.rs:82` — The pattern of iterating over all tasks and applying a tag_parser operation to their bodies is reimplemented here instead of reusing the identical pattern from delete.rs. Both iterate, read body, transform via tag_parser, check for changes, and write back. Extract a shared helper function `apply_tag_edit_to_all_tasks(ectx: &EntityContext, edit_fn: impl Fn(&str) -> String) -> Result<()>` in tag/shared.rs or task/shared.rs. Call it from both delete.rs as `apply_tag_edit_to_all_tasks(&ectx, |body| tag_parser::remove_tag(body, &slug))` and from update.rs as `apply_tag_edit_to_all_tasks(&ectx, |body| tag_parser::rename_tag(body, &old_slug, &normalized))`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:2242` — Test docstring claims 'in every wire shape and every ref format', but the ref_form iteration omits uppercase forms (tag_id.to_uppercase() and short.to_uppercase()). The resolver unit tests in task/tags.rs prove uppercase ULIDs and short IDs resolve correctly; this integration test should exercise the same forms through the MCP boundary to match its stated scope. Add tag_id.to_uppercase() and short.to_uppercase() (and optionally format!("^{}", short.to_uppercase())) to the ref_form iteration, or revise the docstring to clarify the test scope.

## Review Findings (2026-07-30 12:30)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:43` — pub struct KanbanTool is a public type with non-empty representation but does not implement Debug — the validator rule requires all public types with non-empty representation to implement Debug. Add Debug to the derive macro: `#[derive(Debug, Default)]`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:133` — The `schema()` and `schema_full()` methods (lines 133–137 and 139–143, respectively) are near-verbatim duplicates. Both call `kanban_operations()` and pass it to a schema generation function, differing only in which function to call (`generate_kanban_mcp_schema` vs `generate_kanban_mcp_schema_full`). Extract a shared helper parameterized by the schema generator function. Extract a helper method `fn generate_schema(&self, full: bool) -> serde_json::Value` that conditionally calls the appropriate schema generator, or use a callback: `fn build_schema<F>(&self, generator: F) -> serde_json::Value where F: Fn(...) -> Value`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:184` — The MCP server handling block in `init()` (lines ~184–193) is near-verbatim with the same block in `deinit()` (lines ~276–285). Both check if `mcp_server` is injected, call a mirdan registration/unregistration function, check for errors via `applier_error()`, and extend results. They differ only in function names (`register_mcp_server` vs `unregister_mcp_server`) and error-handling strategy (early return vs push-and-continue). Extract a helper function parameterized by operation and error-handling mode. Extract a helper `apply_mcp_server_lifecycle(scope, name, reporter, register: bool, on_error: impl Fn()) -> Vec<InitResult>` that dispatches to the appropriate mirdan function and error strategy. Alternatively, pass a closure for the operation.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:207` — The `init()` method's scope-conditional block (lines 207–242) is near-verbatim with the same block in `deinit()` (lines 303–338). Both check the scope, get the current directory, handle errors, join `.kanban`, call a merge-driver function, emit events, and push results. They differ only in function names (`register_merge_drivers` vs `unregister_merge_drivers`) and string literals (verbs and messages). This should be extracted to a shared helper function parameterized by operation and strings. Extract a helper function `apply_merge_driver_lifecycle(scope, reporter, name, register: bool, verb: &str, operation_desc: &str) -> Vec<InitResult>` and call it from both `init()` and `deinit()`. Pass a closure or enum to dispatch between register and unregister operations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:233` — Error message starts with capital letter — the rule requires error Display messages to be lowercase with no trailing punctuation. Change message to lowercase: `"failed to parse kanban operation: {}"`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:287` — The `is_task_modifying_operation` function uses a massive `matches!` statement enumerating 12 (verb, noun) pairs over a known, finite set (the operation enum variants). Each arm has identical logic (all return true). This is a perfect candidate for extracting to a constant set and checking membership, making the code more maintainable and emphasizing that this is data (a set of operation types to track) rather than control flow. Extract the 12 pairs to a constant set: `const TASK_MODIFYING_OPERATIONS: &[(Verb, Noun)] = &[(Verb::Add, Noun::Task), (Verb::Update, Noun::Task), …];` Then replace the matches! block with a single membership check: `TASK_MODIFYING_OPERATIONS.contains(&(verb, noun))`. This makes the enumeration of tracked operations explicit, declarative, and easy to extend without touching control flow.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:289` — Error message starts with capital letter — the rule requires error Display messages to be lowercase with no trailing punctuation. Change message to lowercase: `"cannot determine current directory"`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:296` — Error message starts with capital letter — the rule requires error Display messages to be lowercase with no trailing punctuation. Change message to lowercase: `"no .kanban directory found"`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:301` — Error message starts with capital letter — the rule requires error Display messages to be lowercase with no trailing punctuation. Change message to lowercase: `format!("failed to register merge drivers: {e}")`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:341` — Error message starts with capital letter — the rule requires error Display messages to be lowercase with no trailing punctuation. Change message to lowercase: `format!("failed to remove merge drivers: {e}")`.

## Review Findings (2026-07-30 13:31)

- [x] `apps/kanban-cli/src/commands/serve.rs:159` — Error message starts with uppercase letter. Display messages on errors must be lowercase, no trailing punctuation. Change `"Unknown tool:"` to `"unknown tool:"`.
- [x] `apps/kanban-cli/src/commands/serve.rs:307` — Error message at line 307 has 'Unknown tool' capitalized while other error messages in the same file—'cannot read cwd' (line 261) and 'failed to parse kanban operation' (line 293)—are lowercase. The change description states 'Error Display messages lowercased across the whole file,' but this message was not lowercased, creating an inconsistency. Change line 307 from `format!("Unknown tool: {}"` to `format!("unknown tool: {}"` to match the lowercase pattern applied to other error messages in the refactor.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:154` — Public trait method `fn name()` in Initializable impl lacks documentation. Trait implementations should document their methods for consistency. Add doc comment explaining the method returns the kanban tool's identifier.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:156` — Public trait method `name()` in `Initializable` impl lacks doc comment, while later methods in same impl have them (line 172+), creating inconsistency. Add doc comment explaining the method, or rely on trait documentation being sufficient for all simple forwarding methods.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:158` — Public trait method `fn display_name()` in Initializable impl lacks documentation. Trait implementations should document their methods. Add doc comment explaining this is the human-readable name for the kanban tool.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:160` — Public trait method `display_name()` in `Initializable` impl lacks doc comment. Add doc comment or ensure consistent treatment across trait impl.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:162` — Public trait method `fn category()` in Initializable impl lacks documentation. Trait implementations should document their methods. Add doc comment explaining this categorizes the kanban tool as part of the 'tools' category.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:164` — Public trait method `category()` in `Initializable` impl lacks doc comment. Add doc comment or ensure consistent treatment across trait impl.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:166` — Public trait method `fn priority()` in Initializable impl lacks documentation. Trait implementations should document their methods. Add doc comment explaining this returns the initialization priority used in the lifecycle sequencing.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:168` — Public trait method `priority()` in `Initializable` impl lacks doc comment. Add doc comment or ensure consistent treatment across trait impl.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:182` — Public trait method `description()` in `McpTool` impl lacks doc comment. Add doc comment explaining the tool description.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:351` — Public trait method `execute()` in `McpTool` impl lacks doc comment, while later trait methods in the `execute_operation` function (outside impl) have them. Add doc comment explaining the execute method's behavior and role in dispatching tool calls.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:398` — Public trait method `fn description()` lacks documentation. Trait implementations should document their methods. Add doc comment explaining it returns the tool description from an embedded file.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:402` — Public trait method `fn schema()` lacks documentation. The difference between `schema()` and `schema_full()` is not self-explanatory without explanation. Add doc comment explaining what the compact schema contains and how it differs from `schema_full()`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:406` — Public trait method `fn schema_full()` lacks documentation. The meaning of 'full' and how it differs from `schema()` is not self-explanatory. Add doc comment explaining what fields or details are included in the full schema vs the compact version.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:410` — Public trait method `fn operations()` lacks documentation. It is not immediately clear what operations are returned or their purpose. Add doc comment explaining this method returns the list of kanban operations supported by the MCP tool.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:414` — Public trait method `async fn execute()` lacks documentation. This is a large, complex method implementing the tool execution logic with multiple steps. Add comprehensive doc comment explaining the method's purpose, input/output format, and key behaviors (e.g., plan data attachment for task-modifying operations).
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:415` — Batch operation response format changed without test coverage. When a batch contains task-modifying operations and plan data is included, responses that previously returned `[{...}, {...}]` are now wrapped as `{"result": [...], "_plan": {...}}`. This breaks the response shape contract for clients expecting array responses from batch operations. Either (1) add test coverage for batch operations with task-modifying operations to document the new format as intentional, or (2) modify the wrapping logic to append `_plan` to array responses while preserving the array format (e.g., `response = json!({"entries": response, "_plan": plan})` or similar), maintaining backward compatibility.

## Review Findings (2026-07-30 15:08)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:194` — The TASK_MODIFYING_OPERATIONS list specifies 12 operations that attach plan data in the execute function (lines 390, 411). Plan-data write-back is tested (executed) for all 12, but read-back (verification that plan contains affected_task_id) is verified for only 4 of 12 operations. The write-side is comprehensive; the read-side (proving 'round-trip') is incomplete. Operations add task, delete task, move task, complete task, unassign task, untag task, update comment, and delete comment attach plan data but are never tested to read it back. Add read-back tests for all 8 missing operations following the pattern of test_update_task_plan_carries_affected_task_id: each test should execute the operation and assert data["_plan"]["_meta"]["affected_task_id"] equals the affected task or comment's ID, proving the plan round-trip for all 12 task-modifying operations.

## Review Findings (2026-07-30 16:06)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:410` — The `run_lifecycle` function has 4-level nesting depth, exceeding the 3-level threshold. The if statement checking `spec.abort_on_mcp_error` sits inside the `Some(err) =>` arm of a match statement, which sits inside an `if let` pattern, creating a chain: if let → match → Some arm → if. Flatten the nesting by extracting the MCP error handling into a separate helper method, or replace the match with an if let: `if let Some(err) = applier_error(&mcp) { results.push(...); if spec.abort_on_mcp_error { return results; } } else { results.extend(mcp); }`. This reduces the if statement to 3-level depth.