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