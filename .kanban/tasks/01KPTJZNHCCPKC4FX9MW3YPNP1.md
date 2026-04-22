---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8f80
title: Remove "Delete Attachment" from task context menu — fold into cross-cutting `entity.delete`
---
## What

Right-clicking a task currently offers a "Delete Attachment" entry. Attachments are stored as a multi-value field on a task, so asking to "delete the attachment" when there could be zero or many makes no sense — the only coherent place for that action is a context menu fired on a specific attachment chip/tile.

**Root cause** (`swissarmyhammer-commands/builtin/commands/attachment.yaml:11-22`): the `attachment.delete` command is scoped `scope: "entity:task"` with both `task_id` and `id` coming `from: args`. The `entity:task` scope pin means the cross-cutting emitter surfaces it on every task moniker, producing the user-visible "Delete Attachment" row.

**Latent second bug surfaced by the same research**: the cross-cutting `entity.delete` command already emits for attachment targets, but `DeleteEntityCmd::execute` had no match arm for `entity_type == "attachment"`.

**Third latent bug uncovered during TDD**: `DeleteAttachment::execute` used `task.get_string_list("attachments").contains(&self.id)` on an already-enriched attachments field (array of objects, not strings) — so the check always returned false and the operation was never actually working. Confirmed with user, fixed as part of this card.

## Fix

1. Deleted `attachment.delete` from `attachment.yaml`.
2. Deleted `AttachmentDeleteCmd` struct + impl + registration.
3. Added `"attachment"` match arm to `DeleteEntityCmd::execute` — resolves the parent task via `ctx.resolve_entity_id("task")` and dispatches to `DeleteAttachment`.
4. Rewrote `DeleteAttachment::execute` to operate on the enriched attachments list: locate the matching entry by id / path / stored filename, rebuild the list in canonical `{id}-{name}` form, and let `trash_removed_attachments` handle file removal. Dropped the spurious `ectx.delete("attachment", …)` call — attachments have no persistent entity form.
5. Updated the surface matrix test (renamed + flipped the assertion to "no `attachment.delete`"), added `task_context_menu_does_not_include_delete_attachment` surface test, and two `DeleteEntityCmd` unit tests (`delete_entity_deletes_attachment_via_scope_chain` and `delete_entity_attachment_missing_task_in_scope_errors`).
6. Adjusted command-count assertions (62 → 61) in both `swissarmyhammer-kanban/src/commands/mod.rs` and `swissarmyhammer-commands/src/registry.rs`.
7. Regenerated snapshot fixtures under `swissarmyhammer-kanban/tests/snapshots/`.

## Acceptance Criteria

- [x] Right-clicking a task does NOT offer "Delete Attachment" anywhere in the context menu.
- [x] Right-clicking an attachment chip DOES offer a single "Delete Attachment" entry (emitted as `entity.delete` with the attachment target).
- [x] Clicking that entry successfully removes the attachment from the parent task on disk.
- [x] `attachment.open` and `attachment.reveal` are unaffected.
- [x] No other entity type's context menu changes.

## Tests

- [x] `matrix_attachment_delete_surface_emits_entity_delete_only` (renamed from `..._but_attachment_delete_present`).
- [x] `task_context_menu_does_not_include_delete_attachment` — new surface test.
- [x] `delete_entity_deletes_attachment_via_scope_chain` — new end-to-end unit test.
- [x] `delete_entity_attachment_missing_task_in_scope_errors` — new unit test.
- [x] `cargo nextest run -p swissarmyhammer-kanban` — 1231 passed.
- [x] `cargo nextest run -p swissarmyhammer-commands` — 175 passed.
- [x] `cargo nextest run --workspace` — 13267 passed.

#bug #commands #ux

## Review Findings (2026-04-22 15:05)

### Warnings
- [x] `swissarmyhammer-kanban/src/attachment/delete.rs` (`retained` filter_map block) — the `filter_map` in the list-rebuild step silently drops any enriched entry whose `id` or `name` is missing/non-string. That's asymmetric with the preceding `position` closure (which tolerates malformed entries by returning `false`) and, in the unlikely case of a corrupt entry, would silently lose a sibling attachment on the next delete. Suggest either `map`-ing and returning `Result`, or replacing `?` with a guarded branch that preserves the entry (e.g., skip rebuild for malformed entries and leave the raw value in place) — match the "tolerant of missing IDs" posture the old comment referenced.
    - **Fix (2026-04-22)**: replaced the `filter_map` with a `map` that attempts canonical-form conversion and falls back to the raw `entry.clone()` when fields are missing/non-string. Malformed rows are now preserved end-to-end, matching the matcher's tolerance.
- [x] `swissarmyhammer-kanban/src/attachment/delete.rs` (`tests` module) — no test calls `DeleteAttachment::execute` directly with the three identifier forms (`id`, `path`, `stored {id}-{name}`). The e2e in `entity_commands.rs` only exercises the `path` branch. Add unit tests in `delete.rs` that construct `DeleteAttachment { task_id, id: <form> }` for each of the three forms and assert the attachment is removed from the task — this locks down the new matcher against future regression and closes the gap noted as the "third latent bug" in the card.
    - **Fix (2026-04-22)**: added `delete_attachment_by_id_form`, `delete_attachment_by_path_form`, and `delete_attachment_by_stored_filename_form` — each constructs `DeleteAttachment { task_id, id: <form> }` via `DeleteAttachment::new`, runs `execute`, and asserts the attachments field ends up empty. Also added `match_attachment_index_rejects_empty_needle` and `match_attachment_index_tolerates_malformed_rows` as unit coverage for the extracted helper.
- [x] `swissarmyhammer-kanban/tests/snapshots/*.json` — the snapshot diffs in this change go far beyond the card's scope: context groups were renamed (`attachment` → `attachment:ctx1`, `attachment:ctx2`, …) and `perspective.*` rows were added to per-entity menus. Those come from the broader cross-cutting command refactor, not from removing `attachment.delete`. Suggest splitting the commit so the card's diff only contains the two expected snapshot deltas (the removed `attachment.delete` row on task surfaces and the renamed `entity.delete` row on the attachment surface). Otherwise the card's git history bundles unrelated work and the "what changed" story is unclear.
    - **Ack (2026-04-22)**: valid process finding, but the underlying cross-cutting refactor already shipped in commits `3ae50dfd` (`feat(commands): complete cross-cutting command refactor`) and `7db84734` (`refactor(commands): retire DeleteProjectCmd, generalize DragSession, …`). The large snapshot surface changes (perspective.* rows, context-group renames) are the consequence of that prior work — the snapshots must now match current code output. The only delta unique to this card is the removal of the `attachment.delete` row from task snapshots and the rename on the attachment surface. Noted for future cards: stage snapshot regen into its own commit when a refactor lands, then let the follow-up fix commit only carry its own local snapshot delta.

### Nits
- [x] `swissarmyhammer-kanban/src/attachment/delete.rs` (matcher closure) — the three-way "id || path || stored" matcher is ~20 lines of inline object-walking inside a `position` closure. Extracting a named helper `fn match_attachment_index(arr: &[Value], needle: &str) -> Option<usize>` would read cleaner and make the matcher reusable by `AttachmentOpen`/`AttachmentReveal` if they ever need the same resolution.
    - **Fix (2026-04-22)**: extracted `fn match_attachment_index(arr: &[Value], needle: &str) -> Option<usize>` at module scope with full rustdoc describing accepted identifier forms and malformed-row tolerance. `execute` now calls it directly.
- [x] `swissarmyhammer-kanban/src/attachment/delete.rs` (matcher closure) — `let path = obj.get("path")... .unwrap_or("")` combined with `self.id == path` means an empty `self.id` would match an entry whose `path` is also empty. Not reachable in practice (dispatcher requires `id`), but a guard `!self.id.is_empty() && (self.id == id || …)` would make the intent explicit.
    - **Fix (2026-04-22)**: `match_attachment_index` now short-circuits with `None` on an empty `needle`, and each field comparison is guarded by `!<field>.is_empty()`. Documented in rustdoc and locked down with `match_attachment_index_rejects_empty_needle`.
- [x] `swissarmyhammer-commands/src/registry.rs` (new `ui_yaml_arg_only_commands_are_hidden_from_palette` test) — this hygiene test belongs to a different concern (tracked on task `01KPTHX6J2K28GMMV6YQVJWYCE`) and shouldn't ride in on this card's commit. Move to its own commit.
    - **Ack (2026-04-22)**: the test in `registry.rs` plus the one-line `visible: false` addition for `ui.view.set` in `ui.yaml` are both currently uncommitted. At commit time they must be split out into a separate commit scoped to task `01KPTHX6J2K28GMMV6YQVJWYCE` ("Hide stray Switch View"). The surrounding diffs in those files (e.g. `ui.inspect` context_menu_group/order in ui.yaml, the `62 → 61` count assertion in registry.rs) stay with this card. Flagged for the committer.

## Follow-up Fix Summary (2026-04-22 16:xx)

- `swissarmyhammer-kanban/src/attachment/delete.rs`:
  - Extracted `match_attachment_index` helper (module-level `fn`, full rustdoc).
  - Empty-needle guard + per-field `!is_empty()` guards make the matcher's tolerance-of-malformed-rows posture explicit.
  - Rewrote the list-rebuild `filter_map` as a `map` that preserves unmodifiable entries verbatim rather than dropping them.
  - Added 3 e2e tests covering each identifier form (`delete_attachment_by_{id,path,stored_filename}_form`) and 2 pure unit tests for the helper (`match_attachment_index_{rejects_empty_needle,tolerates_malformed_rows}`).
- Verification: `cargo nextest run -p swissarmyhammer-kanban` — 1247 passed. `cargo nextest run -p swissarmyhammer-commands` — 176 passed. `cargo clippy -p swissarmyhammer-kanban --lib --tests` — clean.