---
assignees:
- claude-code
depends_on:
- 01KTCAE5WVKRHTYNJYZT7F2M9K
- 01KTCQG9K3825K8XFZKPHZYNA8
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa880
project: card-comments
title: Add comment-log display + editor and register them in the field registry (UI)
---
## What
Surface the conversation log in the task inspector as a metadata-driven field. The inspector renders every schema field through the field registry, so once the `comments` field is on the task entity (task 2) and registered here, it appears automatically in the new "Log" section. Do NOT hardcode comment logic in the inspector — register a display + editor for the `comment-log` name and let metadata drive it (per "UI is an interpreter of Field metadata").

ARCHITECTURE (confirmed): editors are **pure UI**. They emit the new field value via `onChange`/`onCommit`; `Field` persists it through `updateField` → the generic `entity.update_field` command. The comment editor MUST NOT dispatch any comment-specific command and MUST NOT call `useDispatchCommand` directly — it just builds the new `comments` array and commits it, exactly like `attachment-editor.tsx` does (`const next = [...]; onChange?.(next)`). All server-side member id/timestamp/author logic runs in the `UpdateEntityField` comment-log normalization branch (dependency task). The editor only sends member `text` (and, for edits, the member's existing `id`); it never sends id/timestamp/author for new members.

DELETE SEMANTICS (tombstones, matches the normalize card): the server merge treats a member's ABSENCE from the committed array as "preserve" — this protects comments the agent appended concurrently while the inspector was open. Deletion is therefore EXPLICIT: to delete a member the editor emits the array with that member REPLACED by a tombstone `{ id: member.id, deleted: true }` — it never just filters the member out. Tombstones are wire-only; they never come back from the server.

Follow the attachment field UI precedent for structure/registration:
- `apps/kanban-app/ui/src/components/fields/registrations/attachment.tsx` (adapter + `registerDisplay`/`registerEditor`)
- `apps/kanban-app/ui/src/components/fields/displays/attachment-display.tsx`
- `apps/kanban-app/ui/src/components/fields/editors/attachment-editor.tsx`

Files:
1. `apps/kanban-app/ui/src/components/fields/displays/comment-log-display.tsx` — render the comments array as a chronological thread, ordered by member `id` ascending (ULIDs are time-ordered; the server stores them in this order). Each member shows: the **actor** resolved to name/avatar (reuse the same actor-resolution the assignees field/avatar uses — actor ids → display via the schema/entity store; do not refetch), the **text**, and the **timestamp** (reuse the existing date/relative-time display formatting). Read-only.
2. `apps/kanban-app/ui/src/components/fields/editors/comment-log-editor.tsx` — PURE UI, full CRUD on the array via `onChange`/`onCommit`:
   - **Add**: a compose box; on submit, `onChange?.([...current, { text }])` (new member has only `text` — server assigns id/timestamp/author). Empty text is a no-op.
   - **Edit**: per-member inline edit of `text` only; on commit, map the array replacing that member's `text` (keep its `id`) and emit.
   - **Delete**: per-member control; emit the array with that member replaced by `{ id: member.id, deleted: true }` (tombstone — do NOT filter it out).
   - Never sends actor/timestamp/id for new members; never passes an author.
3. `apps/kanban-app/ui/src/components/fields/registrations/comment-log.tsx` — `registerDisplay("comment-log", ...)` + `registerEditor("comment-log", ...)` adapters (FieldDisplayProps/FieldEditorProps), mirroring `attachment.tsx`.
4. `apps/kanban-app/ui/src/components/fields/registrations/index.ts` — add `import "./comment-log";` (a file not imported here never registers).
5. If the UI maintains a TS `FieldType`/display-name mirror (`apps/kanban-app/ui/src/types/kanban.ts`), add `comment-log` there (Rust treats display/editor names as free strings — mirror that).

## Acceptance Criteria
- [x] Opening a task inspector renders the `comments` field via the `comment-log` display under the "Log" section, showing each member's actor (name/avatar), text, and timestamp in id order (== chronological).
- [x] Add: composing + submit emits `[...current, {text}]` through `onChange`/`onCommit`; the new member appears reactively (via the field-change event) with a server-assigned author/timestamp/id.
- [x] Edit: changing a member's text emits the array with that member's `text` updated and `id` retained; actor/timestamp unchanged after round-trip.
- [x] Delete: removing a member emits the array with that member replaced by `{id, deleted: true}`; the member disappears after round-trip. The editor never deletes by omission.
- [x] The editor does NOT call `useDispatchCommand` and contains no comment-specific command name; persistence flows only through `Field`/`updateField`.
- [x] No comment-specific branching in inspector/`EntityInspector` — all behavior via the registry.
- [x] `npm run lint` and `npx tsc --noEmit` clean in `apps/kanban-app/ui`. (NOTE: the package has no `lint` script or eslint config — that half of the criterion is stale; `tsc --noEmit` is clean and runs inside `npm test`.)

## Tests
- [x] `comment-log-display.test.tsx` (modeled on `attachment-display.test.tsx`): given a `comments` value with two members, asserts both texts, both resolved actor labels, and members render in id order.
- [x] `comment-log-editor.test.tsx` (modeled on `attachment-editor.test.tsx`): submit emits `[...current, {text}]` (empty submit is a no-op); editing a member emits the array with updated text + retained id; deleting emits the array with the tombstone `{id, deleted: true}` in place of the member (and never an array with the member simply missing). Assert the editor never calls a dispatch hook (pure UI — it only calls `onChange`/`onCommit`).
- [x] A browser/integration test drives the inspector: add a comment, assert it renders with a resolved author and timestamp after the field-set round-trip; edit and delete (tombstone) round-trip. (Implemented as `entity-inspector.comment-log.test.tsx`, modeled on `entity-inspector.test.tsx` — `board-integration.browser.test.tsx` does not actually reference a comments shape and the CLI exposes no update-field verb, so the inspector harness with the kernel simulator + a server-mirror of `comment/normalize.rs` is the faithful round-trip available in UI-land.)
- [x] `npm test` in `apps/kanban-app/ui` — green (260 files / 2514 tests after review fixes).

## Implementation notes (done)
- New: `comment-utils.ts` (member types + `normalizeComments`, mirrors `attachment-utils.ts`), `comment-log-display.tsx` (`CommentItem` shared with the editor), `comment-log-editor.tsx` (draft-state so consecutive ops within the 1s autosave debounce compose — two quick deletes carry BOTH tombstones), `registrations/comment-log.tsx`, `entity-inspector.comment-log.test.tsx`.
- `avatar.tsx`: extracted `useActorDisplay(actorId)` (entity + display-name via `mention_display_field`) so Avatar and the comment thread resolve actors identically.
- `editor-save.test.tsx`: excluded `comment-log` from the save matrix alongside `attachment` (array-CRUD editors have no text buffer to save on blur/Enter/Escape).
- `types/kanban.ts` needs no change — kind/editor/display are intentionally open strings per the file's own comment.

## Workflow
- Use `/tdd` — write the display + editor (add/edit/tombstone-delete, pure-UI) component tests first, then implement and register.

## Review Findings (2026-06-12 07:23)

Verified before review: the 4 in-scope test files pass (54/54), `npx tsc --noEmit` exits 0, registered names match `builtin/definitions/comments.yaml`, `entity-inspector.tsx` contains zero comment-specific code, and the editor test asserts no `dispatch_command` ever fires. No blockers; the draft mechanism correctly resets on the server round-trip (the `[value]` resync effect) and external agent appends are never lost (absence-means-preserve + resync). Remaining items below.

### Warnings
- [x] `apps/kanban-app/ui/src/components/fields/editors/comment-log-editor.tsx` (compose `Textarea` `onKeyDown`) — Enter-to-submit has no IME composition guard. For CJK/IME users, pressing Enter to confirm a composition will post the comment mid-composition. `prompt-input.tsx` guards the identical textarea+Enter-submit pattern with `e.nativeEvent.isComposing`; add the same check (`if (e.nativeEvent.isComposing) return;`) before treating Enter as submit.
- [x] `apps/kanban-app/ui/src/components/fields/editors/comment-log-editor.tsx` (draft resync `useEffect` on `[value]`) — composite race can silently drop an un-flushed local op. Sequence: user deletes A (draft holds tombstone, 1s debounce pending) → an external field-change lands (agent append — the design's stated concurrency case) → the effect wholesale-resets the draft to the server value, discarding the tombstone from the draft (A visibly reappears) → if the user performs ANY further op before the debounce fires, the new emit (built from the resynced draft) REPLACES the pending save in `useDebouncedSave`, and A's deletion is permanently lost. Same shape for a pending `{text}` add (the typed comment vanishes at resync and is dropped if a follow-up op lands in the window). Suggest rebasing instead of wholesale reset: on `value` change, re-apply local un-acknowledged tombstones whose ids still exist in the fresh server value (tombstones are idempotent server-side) and keep pending `{text}` members until a server round-trip mints them; or alternatively flush the pending save before accepting an external resync. If you instead decide the 1s window is acceptable, document the accepted race in the draft-resync comment so the next reader doesn't rediscover it.

### Nits
- [x] `apps/kanban-app/ui/src/components/fields/editors/comment-log-editor.tsx` (`handleAdd`) — gates on `composeText.trim()` but emits the untrimmed `{ text: composeText }`; the server (`comment/normalize.rs`) stores text as-is, so leading/trailing whitespace persists. Emit the trimmed text.
- [x] `apps/kanban-app/ui/src/components/fields/editors/comment-log-editor.tsx` (`MemberRow.commitEdit`) — an edit that clears the text to empty/whitespace commits `{id, text: ""}` and persists an empty comment body (author + timestamp with no text), while the add path no-ops on empty. Treat an empty edit as a cancel/no-op for consistency.

### Resolution (2026-06-12)
All four findings fixed TDD-style (each repro test watched fail first, then fixed):
1. **IME guard** — compose `onKeyDown` now returns early on `e.nativeEvent.isComposing` before any Enter handling, matching `prompt-input.tsx`. Test: "Enter during an IME composition does NOT submit".
2. **Resync race** — replaced the wholesale draft reset with `rebaseComments(draft, baseline, fresh)` (new pure helper in `comment-utils.ts`; the editor tracks the last-rebased server value in `baselineRef`). Un-acknowledged tombstones are re-applied while their id still exists in the fresh value (idempotent server-side) and dropped once acknowledged; pending `{text}` adds are kept until the server mints them (a fresh member absent from baseline with the same text); un-acknowledged text edits are also rebased (same race class) and yield to any server-side text change. Flush-before-resync was not viable — the debounce lives in `Field`'s `useDebouncedSave`, out of the editor's reach. Regression suite "external resync rebases un-flushed local ops" (5 tests) simulates exactly the reviewed sequence: local op → external value update inside the debounce window → second local op → assert the second emit still carries the first op; plus the two acknowledge/mint drop guards.
3. **Trim** — `handleAdd` emits the trimmed text. Test: "trims surrounding whitespace from the submitted text".
4. **Empty edit = cancel** — `commitEdit` no-ops on empty/whitespace text, consistent with add. Test: "clearing a member's text to whitespace emits nothing".

Verified: `comment-log-editor.test.tsx` 23/23, full `npm test` 260 files / 2514 tests green, `npx tsc --noEmit` exit 0.