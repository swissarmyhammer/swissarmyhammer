---
assignees:
- claude-code
depends_on:
- 01KN4NEJ6J90Q1FDRP9JCS0R5K
position_column: done
position_ordinal: ffffffffffffffffb680
title: Attachment display components (attachment + attachment-list)
---
## What

Register `"attachment"` and `"attachment-list"` display components in the field display registry. These render enriched attachment metadata objects returned by the entity layer.

### Data shape (from entity layer read)
Each attachment in the array is:
```json
{ "id": "01abc", "name": "screenshot.png", "size": 12345, "mime_type": "image/png", "path": "/absolute/path/to/.kanban/tasks/.attachments/01abc-screenshot.png" }
```

### Display behavior
- **`attachment-list`** (multiple): renders a list of attachment items
- **`attachment`** (single): renders one attachment item
- Each item shows: filename, human-readable file size (e.g., "12.1 KB"), file type icon
- Empty state: subtle "No attachments" text

### File type icons (lucide-react, already installed)
Generate the appropriate lucide icon based on MIME type — no file content loaded, no asset protocol needed:
- `image/*` → `FileImage`
- `video/*` → `FileVideo`
- `audio/*` → `FileAudio`
- `text/*`, `.md`, `.txt` → `FileText`
- `application/javascript`, `application/typescript`, `.js`, `.ts`, `.py`, `.rs`, `.go`, etc. → `FileCode`
- `application/pdf` → `FileText`
- `.csv`, `.xls`, `.xlsx` → `FileSpreadsheet`
- `.zip`, `.tar`, `.gz`, `.7z` → `FileArchive`
- fallback → `File`

Use the existing `icons[kebabToPascal(name)]` dynamic lookup pattern already used in `entity-icon.tsx`, `entity-inspector.tsx`, and `entity-card.tsx`.

### Registration
Follow the existing pattern in `kanban-app/ui/src/components/fields/registrations/`:
- Create `registrations/attachment.tsx` with adapter components
- Register via `registerDisplay("attachment", ...)` and `registerDisplay("attachment-list", ...)`
- Import in `registrations/index.ts`

### Files to create
- `kanban-app/ui/src/components/fields/displays/attachment-display.tsx` — display components
- `kanban-app/ui/src/components/fields/displays/attachment-display.test.tsx` — vitest tests
- `kanban-app/ui/src/components/fields/registrations/attachment.tsx` — registry adapters

### Files to modify
- `kanban-app/ui/src/components/fields/registrations/index.ts` — import attachment registration

## Acceptance Criteria
- [ ] `"attachment-list"` display registered and renders array of attachment metadata
- [ ] `"attachment"` display registered and renders single attachment metadata
- [ ] Filenames displayed with human-readable file sizes
- [ ] Correct lucide icon generated per MIME type / extension
- [ ] Empty state handled gracefully

## Tests (vitest + React Testing Library)
- [ ] Test: renders attachment list with multiple items — filenames and sizes in DOM
- [ ] Test: renders single attachment
- [ ] Test: empty array renders empty state
- [ ] Test: image MIME type gets FileImage icon
- [ ] Test: code file gets FileCode icon
- [ ] Test: unknown type gets fallback File icon
- [ ] Run: `pnpm test` in `kanban-app/ui/` — all pass