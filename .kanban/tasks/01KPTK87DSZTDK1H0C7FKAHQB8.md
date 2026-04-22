---
assignees:
- claude-code
position_column: todo
position_ordinal: fe80
title: 'Bug: dragging a file onto task inspector attachment field corrupts the task ("name.lastIndexOf" crash, wedges inspector)'
---
## What

**Repro**: Open a task in the inspector. Drag a file from the OS onto the attachment field. The drop shows the toast "Something went wrong — undefined is not an object (evaluating 'name.lastIndexOf')" and the inspector becomes permanently wedged: closing and re-inspecting the same task renders ONLY the error boundary, not the inspector content.

**Root cause** (traced end-to-end):

1. `kanban-app/ui/src/components/fields/displays/attachment-display.tsx:298-301` — `AttachmentListDisplay.handleDrop` receives temp paths from `useFileDrop` (which has already copied the file to a temp location via `save_dropped_file`) and calls `onCommit([...current, ...paths])`. It appends **raw string paths** into the attachments list alongside proper `AttachmentMeta` objects.
2. The adapter in `kanban-app/ui/src/components/fields/registrations/attachment.tsx:18-34` wires `onCommit` through the standard field-update path, which persists the malformed mixed array to disk via `entity.update_field`.
3. On next render, `AttachmentItem` (`attachment-display.tsx:189-200`) assumes every element is an `AttachmentMeta` and calls `getFileIcon(attachment.mime_type, attachment.name)`. For a raw string path, `.mime_type` and `.name` are `undefined`, and `getExtension(undefined)` at `attachment-display.tsx:82` does `undefined.lastIndexOf(".")` — the exact error the user sees.
4. The inspector's React error boundary catches the render-time throw and replaces the whole inspector with "Something went wrong". Because the corruption is **persisted on disk**, subsequent inspections re-hit the crash.

**The correct path already exists** (and is used by dropping a file onto a task card on the board): `kanban-app/ui/src/lib/drag-session-context.tsx` exposes `startFileSession(path)` + `completeFileSession(targetMoniker)` which dispatches `drag.start` / `drag.complete`, routing through `DragCompleteCmd` (`swissarmyhammer-kanban/src/commands/drag_commands.rs:394-425` `DragSource::File` arm) → `PasteMatrix` → `AttachmentOntoTaskHandler` (`swissarmyhammer-kanban/src/commands/paste_handlers/attachment_onto_task.rs`). That handler calls `AddAttachment::new(task_id, name, path)` which correctly mints a fresh attachment entity with `id`, resolved `name`, `mime_type`, and `size`, and appends it to the task's `attachments` field as a proper `AttachmentMeta`.

So the inspector field drop is the **only** place in the app that circumvents the paste pipeline — and it does so in a way that silently corrupts data. Per the `drag-vs-paste` memory rule: external drag-in is paste, always through `PasteMatrix`. Never commit raw paths through the field-update pathway.

## Approach

Two defects to fix together — routing and data hardening.

### 1. Route field-level drops through the existing paste path

File: `kanban-app/ui/src/components/fields/displays/attachment-display.tsx`

- In `AttachmentDisplay` (line 231) and `AttachmentListDisplay` (line 284), replace the `handleDrop` body that calls `onCommit([...current, ...paths])` with a dispatch through the drag-session hooks.
- Accept the parent `entity` prop (already part of `FieldDisplayProps`, see `kanban-app/ui/src/components/fields/field.tsx:42-49`) so the component has `entity.id` to build a `task:<id>` target moniker.
- New drop handler:
  ```ts
  const { startFileSession, completeFileSession } = useDragSession();
  const handleDrop = useCallback(async (paths: string[]) => {
      if (!entity) return;
      const target = `${entity.entity_type}:${entity.id}`;
      for (const path of paths) {
          await startFileSession(path);
          await completeFileSession(target);
      }
  }, [entity, startFileSession, completeFileSession]);
  ```
  — one paste per file, sequential so the backend sees discrete `AddAttachment` ops (each individually undoable, matching how board-level drops work today).
- Remove the `onCommit` branch from the drop path entirely. The attachment field isn't a simple scalar — it's a list of child entities with their own lifecycle. Field commits are the wrong vehicle.
- Do NOT remove the `registerDropTarget` / `unregisterDropTarget` wiring — the `useFileDrop` provider is still the correct source of OS-drop signals; only the dispatch target changes.

### 2. Forward `entity` through the field adapter

File: `kanban-app/ui/src/components/fields/registrations/attachment.tsx`

- `AttachmentDisplayAdapter` (line 18-24) and `AttachmentListDisplayAdapter` (line 26-34) currently strip `entity` and `field`. Forward `entity`:
  ```tsx
  function AttachmentListDisplayAdapter({ value, mode, entity, onCommit }: FieldDisplayProps) {
      return <AttachmentListDisplay value={value} mode={mode} entity={entity} onCommit={onCommit} />;
  }
  ```
- Add `entity?: Entity` to `AttachmentDisplayProps` and `AttachmentListDisplayProps` (`attachment-display.tsx:42-54`).

### 3. Harden `AttachmentItem` so existing corrupted data doesn't wedge the inspector

File: `kanban-app/ui/src/components/fields/displays/attachment-display.tsx`

Users who hit the bug before the fix still have malformed attachments persisted. Harden the render so they can recover:

- In `AttachmentListDisplay` (line 289) and `AttachmentDisplay` (line 236-238), filter the input: `attachments.filter((a): a is AttachmentMeta => a != null && typeof a === "object" && typeof (a as AttachmentMeta).name === "string")`.
- This preserves valid entries and drops malformed ones from the render, so the inspector loads and the user can edit the task (e.g., add a fresh attachment, delete the task, or clear the field via the editor) to fully clean up.
- Log a single `console.warn("[attachments] dropping malformed entry", entry)` per dropped entry so the bug is observable in OS log if it recurs.

**Do NOT** add a backend data migration in this task — hardening the UI is enough to unwedge users. A separate cleanup task can sweep malformed entries once this is merged (see follow-up note in the task sizing section).

## Acceptance Criteria

- [ ] Dragging a file onto an attachment field in the task inspector successfully adds a properly-shaped `AttachmentMeta` (with `id`, `name`, `mime_type`, `size`, `path`) to the task's attachments list.
- [ ] No "Something went wrong — undefined is not an object (evaluating 'name.lastIndexOf')" toast appears.
- [ ] After the drop, the inspector renders the newly-added attachment row with its icon, name, and size — no inspector crash, no error boundary fallback.
- [ ] Opening a task whose attachments field contains pre-existing malformed entries (from the earlier bug) renders the inspector without crashing; malformed entries are silently skipped, valid ones still render.
- [ ] Dropping onto the inspector dispatches the same `drag.start` → `drag.complete` → `AttachmentOntoTaskHandler` path that board-level drops use (observable via the `entity-field-changed` sequence in the OS log).
- [ ] Undo reverses the attachment-add (comes for free from `AddAttachment`'s existing undo support).
- [ ] No regression on board-card drops (existing drag-on-card flow unchanged).

## Tests

- [ ] New browser-mode test `kanban-app/ui/src/components/fields/displays/attachment-drop.browser.test.tsx`:
  1. Render `AttachmentListDisplay` inside a `FileDropProvider` + mock `DragSessionProvider`.
  2. Register a drop target, fire a synthetic drop with `paths = ["/tmp/a.png", "/tmp/b.pdf"]`.
  3. Assert `startFileSession` is called once per path and `completeFileSession` is called with `target = "task:<entity.id>"` once per path.
  4. Assert `onCommit` is NEVER called with an array containing string entries (this is the regression guard for the exact bug).
- [ ] New unit test `attachment_item_handles_malformed_entry` in `kanban-app/ui/src/components/fields/displays/attachment-display.test.tsx`:
  1. Render `AttachmentListDisplay` with `value = [validMeta, "/tmp/broken.png"]`.
  2. Assert the component renders successfully (no throw, no error boundary).
  3. Assert only the valid meta row is in the DOM.
- [ ] New Rust test in `swissarmyhammer-kanban/src/commands/paste_handlers/attachment_onto_task.rs` tests module (or extend existing test) named `paste_with_repeated_file_paths_adds_distinct_entities`:
  1. Drop the same file onto the same task twice.
  2. Assert the attachments list has two entries, each with a distinct `id`.
  3. This validates that the sequential `startFileSession`/`completeFileSession` loop in the frontend doesn't produce id collisions.
- [ ] Existing tests still pass:
  - `kanban-app/ui/src/components/fields/displays/attachment-display.test.tsx` (all current cases)
  - `kanban-app/ui/src/lib/drag-session-context.test.tsx` (file session dispatch shape unchanged)
  - `swissarmyhammer-kanban/src/commands/paste_handlers/attachment_onto_task.rs` tests (handler semantics unchanged)
- [ ] Run: `cd kanban-app/ui && bun test attachment-display attachment-drop drag-session-context` and `cargo test -p swissarmyhammer-kanban attachment_onto_task` — all passing.

## Workflow

- Use `/tdd`. Start with `attachment_item_handles_malformed_entry` — it reproduces the "re-opening a wedged task crashes" half of the bug. Then write `attachment-drop.browser.test.tsx` asserting the correct dispatch path. Then wire the fixes.
- Do NOT touch `DragCompleteCmd`, `AttachmentOntoTaskHandler`, `AddAttachment`, or `save_dropped_file` — all of those are correct and tested. The bug is entirely in the frontend field drop handler.
- The `onCommit` prop stays on the display components — it's still used by the attachment editor flow for non-drop cases. Only the drop path stops calling it.

## Follow-up (do NOT include in this task)

Once merged, consider a sweeper task: add a one-time backend pass that filters non-object attachment entries from any `task.attachments` list on read. Size permitting, that can ship as a dedicated `#tech-debt` card. #bug #drag-and-drop #frontend #blocker