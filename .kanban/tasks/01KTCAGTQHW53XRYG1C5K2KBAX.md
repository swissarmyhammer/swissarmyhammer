---
assignees:
- claude-code
depends_on:
- 01KTCAE5WVKRHTYNJYZT7F2M9K
- 01KTCQG9K3825K8XFZKPHZYNA8
position_column: todo
position_ordinal: '8680'
project: card-comments
title: Add comment-log display + editor and register them in the field registry (UI)
---
## What
Surface the conversation log in the task inspector as a metadata-driven field. The inspector renders every schema field through the field registry, so once the `comments` field is on the task entity (task 2) and registered here, it appears automatically in the new "Log" section. Do NOT hardcode comment logic in the inspector — register a display + editor for the `comment-log` name and let metadata drive it (per "UI is an interpreter of Field metadata").

ARCHITECTURE (confirmed): editors are **pure UI**. They emit the new field value via `onChange`/`onCommit`; `Field` persists it through `updateField` → the generic `entity.update_field` command. The comment editor MUST NOT dispatch any comment-specific command and MUST NOT call `useDispatchCommand` directly — it just builds the new `comments` array and commits it, exactly like `attachment-editor.tsx` does (`const next = [...]; onChange?.(next)`). All server-side member id/timestamp/author logic runs in the `UpdateEntityField` comment-log normalization branch (dependency task). The editor only sends member `text` (and, for edits, the member's existing `id`); it never sends id/timestamp/author for new members.

Follow the attachment field UI precedent for structure/registration:
- `apps/kanban-app/ui/src/components/fields/registrations/attachment.tsx` (adapter + `registerDisplay`/`registerEditor`)
- `apps/kanban-app/ui/src/components/fields/displays/attachment-display.tsx`
- `apps/kanban-app/ui/src/components/fields/editors/attachment-editor.tsx`

Files:
1. `apps/kanban-app/ui/src/components/fields/displays/comment-log-display.tsx` — render the comments array as a chronological thread (oldest→newest). Each member shows: the **actor** resolved to name/avatar (reuse the same actor-resolution the assignees field/avatar uses — actor ids → display via the schema/entity store; do not refetch), the **text**, and the **timestamp** (reuse the existing date/relative-time display formatting). Read-only.
2. `apps/kanban-app/ui/src/components/fields/editors/comment-log-editor.tsx` — PURE UI, full CRUD on the array via `onChange`/`onCommit`:
   - **Add**: a compose box; on submit, `onChange?.([...current, { text }])` (new member has only `text` — server assigns id/timestamp/author). Empty text is a no-op.
   - **Edit**: per-member inline edit of `text` only; on commit, map the array replacing that member's `text` (keep its `id`) and emit.
   - **Delete**: per-member control; emit the array with that member filtered out.
   - Never sends actor/timestamp/id for new members; never passes an author.
3. `apps/kanban-app/ui/src/components/fields/registrations/comment-log.tsx` — `registerDisplay("comment-log", ...)` + `registerEditor("comment-log", ...)` adapters (FieldDisplayProps/FieldEditorProps), mirroring `attachment.tsx`.
4. `apps/kanban-app/ui/src/components/fields/registrations/index.ts` — add `import "./comment-log";` (a file not imported here never registers).
5. If the UI maintains a TS `FieldType`/display-name mirror (`apps/kanban-app/ui/src/types/kanban.ts`), add `comment-log` there (Rust treats display/editor names as free strings — mirror that).

## Acceptance Criteria
- [ ] Opening a task inspector renders the `comments` field via the `comment-log` display under the "Log" section, showing each member's actor (name/avatar), text, and timestamp in chronological order.
- [ ] Add: composing + submit emits `[...current, {text}]` through `onChange`/`onCommit`; the new member appears reactively (via the field-change event) with a server-assigned author/timestamp/id.
- [ ] Edit: changing a member's text emits the array with that member's `text` updated and `id` retained; actor/timestamp unchanged after round-trip.
- [ ] Delete: removing a member emits the array without it; the member disappears after round-trip.
- [ ] The editor does NOT call `useDispatchCommand` and contains no comment-specific command name; persistence flows only through `Field`/`updateField`.
- [ ] No comment-specific branching in inspector/`EntityInspector` — all behavior via the registry.
- [ ] `npm run lint` and `npx tsc --noEmit` clean in `apps/kanban-app/ui`.

## Tests
- [ ] `comment-log-display.test.tsx` (modeled on `attachment-display.test.tsx`): given a `comments` value with two members, asserts both texts, both resolved actor labels, and timestamps render in chronological order.
- [ ] `comment-log-editor.test.tsx` (modeled on `attachment-editor.test.tsx`): submit emits `[...current, {text}]` (empty submit is a no-op); editing a member emits the array with updated text + retained id; deleting emits the array without the member. Assert the editor never calls a dispatch hook (pure UI — it only calls `onChange`/`onCommit`).
- [ ] A browser/integration test (modeled on `board-integration.browser.test.tsx`, which already references a `comments` field shape) drives the inspector: add a comment, assert it renders with a resolved author and timestamp after the field-set round-trip; edit and delete round-trip.
- [ ] `npm test` in `apps/kanban-app/ui` — green.

## Workflow
- Use `/tdd` — write the display + editor (add/edit/delete, pure-UI) component tests first, then implement and register.