---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffba80
title: Open attachments via double-click and right-click context menu
---
## What

Attachment items are currently inert. Each rendered attachment should be wrapped in a command scope providing attachment-specific commands, using the existing command system — same pattern as entity cards.

### 1. Install `tauri-plugin-opener`
- Rust: add `tauri-plugin-opener = "2"` to `kanban-app/Cargo.toml`, register `.plugin(tauri_plugin_opener::init())` in `kanban-app/src/main.rs`
- JS: add `@tauri-apps/plugin-opener` to `kanban-app/ui/package.json`
- Capabilities: add `"opener:default"` to `kanban-app/capabilities/default.json`

### 2. Register attachment commands in the backend
Add attachment commands to the command registry (`kanban-app/src/commands.rs` or `swissarmyhammer-kanban/src/commands/`):
- `attachment.open` — opens the file at the attachment's `path` with the OS default app
- `attachment.reveal` — reveals the file in Finder

These commands need a target that identifies the attachment (e.g., the `path` field from the enriched metadata).

### 3. Wrap AttachmentItem in a command scope
In `kanban-app/ui/src/components/fields/displays/attachment-display.tsx`:
- Each `AttachmentItem` gets wrapped in a `CommandScopeProvider` that registers `attachment.open` and `attachment.reveal` commands with the attachment's path as context
- Right-click uses the existing `useContextMenu(scopeChain)` hook — same as entity cards
- Double-click resolves and dispatches `attachment.open` — same as the inspect pattern in `focus-scope.tsx`

### 4. Cursor feedback
Add `cursor-pointer` to `AttachmentItem` so it looks clickable.

### Files to modify
- `kanban-app/Cargo.toml` — add `tauri-plugin-opener`
- `kanban-app/src/main.rs` — register opener plugin
- `kanban-app/capabilities/default.json` — add `"opener:default"`
- `kanban-app/ui/package.json` — add `@tauri-apps/plugin-opener`
- `kanban-app/ui/src/components/fields/displays/attachment-display.tsx` — wrap `AttachmentItem` in command scope, add double-click + context menu
- `kanban-app/src/commands.rs` or appropriate command module — register `attachment.open` and `attachment.reveal` commands

## Acceptance Criteria
- [ ] Each attachment item is wrapped in a command scope
- [ ] Right-clicking an attachment shows a native context menu via the command system with "Open" and "Show in Finder"
- [ ] "Open" dispatches `attachment.open` → opens file with OS default app
- [ ] "Show in Finder" dispatches `attachment.reveal` → reveals in Finder
- [ ] Double-clicking an attachment dispatches `attachment.open`
- [ ] Attachment items show pointer cursor on hover

## Tests (vitest + React Testing Library)
- [ ] Test: AttachmentItem renders with cursor-pointer class
- [ ] Test: double-click on AttachmentItem dispatches attachment.open command
- [ ] Test: right-click on AttachmentItem triggers context menu via useContextMenu
- [ ] Run: `pnpm test` in `kanban-app/ui/` — all pass
- [ ] Run: `cargo test -p kanban-app` — all pass