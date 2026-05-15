---
assignees:
- claude-code
depends_on:
- 01KN4NF5Q66P2PXYAKJ7BJZKCS
position_column: done
position_ordinal: ffffffffffffffffffb080
title: Attachment editor component (file picker + remove)
---
## What

Register an `"attachment"` editor component in the field editor registry. This provides the UI for adding and removing file attachments on entities.

### Editor behavior
- Shows existing attachments as a list (filename + size + remove button)
- "Add file" button opens a Tauri file dialog via `@tauri-apps/plugin-dialog` `open()` call
- When user picks a file, append the absolute path to the field value array and fire `onChange`
- Remove button filters the attachment from the array and fires `onChange`
- The entity layer handles the actual file copy on save — the editor just sets paths

### Tauri dialog setup
- Add `@tauri-apps/plugin-dialog` to `kanban-app/ui/package.json` (JS SDK)
- Import `open` from `@tauri-apps/plugin-dialog` in the editor component
- Call `open({ multiple: true, directory: false })` for multi-attachment fields
- Call `open({ multiple: false, directory: false })` for single-attachment fields

### Registration
Follow existing pattern:
- Add editor component in `kanban-app/ui/src/components/fields/editors/attachment-editor.tsx`
- Register via `registerEditor("attachment", ...)` in `registrations/attachment.tsx` (created in display card)
- The registration file already exists from the display card — add the editor registration there

### Files to create
- `kanban-app/ui/src/components/fields/editors/attachment-editor.tsx` — editor component
- `kanban-app/ui/src/components/fields/editors/attachment-editor.test.tsx` — vitest tests

### Files to modify
- `kanban-app/ui/src/components/fields/registrations/attachment.tsx` — add editor registration
- `kanban-app/ui/package.json` — add `@tauri-apps/plugin-dialog` dependency

## Acceptance Criteria
- [ ] `"attachment"` editor registered and renders in entity inspector
- [ ] Existing attachments shown with remove buttons
- [ ] Remove button fires onChange with attachment removed from array
- [ ] "Add file" button calls Tauri `open()` dialog
- [ ] Selected file path appended to field value array via onChange
- [ ] Works for both single and multiple attachment fields

## Tests (vitest + React Testing Library)
- [ ] Test: renders existing attachments with filenames
- [ ] Test: remove button fires onChange with updated array (attachment removed)
- [ ] Test: add button exists and is clickable
- [ ] Test: mock `open()` returning a path → onChange fired with path appended
- [ ] Test: mock `open()` returning null (user cancelled) → no onChange
- [ ] Run: `pnpm test` in `kanban-app/ui/` — all pass