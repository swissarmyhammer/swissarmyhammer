---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: c680
project: null
title: 'Bug: Command palette won''t OPEN by hotkey/execution'
---
## Scope (narrowed 2026-06-06)
This card is now scoped ONLY to the **palette won't OPEN by hotkey/execution** failure — the live frontend keymap binds a different key than the registry advertises, so the advertised shortcut fires nothing, and the open path (command → service flag → ui-state change event → React mounts the overlay) does not complete.

The **"not in the OS menu"** half is handled by **Card A (01KTCQFH7AEQDZD0QETSMCMGP0)** in project `ui-command-cleanup` (adds a `menu` placement to `ui.palette.open` and reconciles the app.command/app.palette id split). Do NOT duplicate that here.

Cross-reference the keystone execution card **01KTCQF326FAQTQMHVV5QPG8VZ** (palette/emit_to execution) — the hotkey-firing failure likely shares its root cause.

This stays in `todo` and is NOT part of the `ui-command-cleanup` project — it is an execution/runtime bug, not a command-definition cleanup. (The duplicate "Navigation menu is blank" card 01KTCRQ1ZNMB5WAZSXT4CQZW6Z has been archived; see Card A.)

## The palette is an overlay, not a window
`<CommandPalette>` mounts via `createPortal(document.body)` inside `<FocusLayer name="palette">`, gated on a per-window `palette_open` flag. The open path is: command → service sets `palette_open=true` → UI-state change event → React re-renders → overlay mounts.

## Current state (this repo — `swissarmyhammer-plugin`)
`ui.palette.open` (`builtin/plugins/ui-commands/index.ts`) is a TS plugin command with `keys:{cua:"Mod+K",vim:":"}` routing to `ui_state…palette.open`. The live frontend keymap (`apps/kanban-app/ui/src/lib/keybindings.ts`) only binds `Mod+Shift+P`/`:`/`/`, so `Mod+K` resolves to nothing — the registry advertises a key the app never listens for. There are also app.command/app.palette duplicates (Card A collapses the id split).

## DECIDED DESIGN (from user — execution half)
- One canonical shortcut opens the palette in all keymap modes, and registry + live keymap agree on it (the survivor id is `ui.palette.open` per Card A; `app.search` opens the same single flag in search MODE as a parameter, not a second flag).
- The command's `keys` must drive the REAL keybinding handler — no separate static binding table that diverges.
- The `palette_open` (+ `palette_mode`) flag is owned by a single service (incumbent `swissarmyhammer-ui-state`, or `window` if the split is accepted) — exactly one owner.

## Acceptance Criteria
- [ ] One canonical shortcut opens the palette in all keymap modes; registry + live keymap agree (no Mod+K vs Mod+Shift+P divergence).
- [ ] Pressing the shortcut completes the open path and mounts the overlay.
- [ ] `palette_open` (+ `palette_mode`) owned by a single service; `open`/`close`/`dismiss` flip it and emit the change.
- [ ] No Rust `Command` impl for palette open/close remains (single TS plugin path).

## Tests
- [ ] Frontend keymap test: the canonical shortcut resolves to the single palette command under the real board scope chain.
- [ ] Service test on the owning service: `open palette` sets the per-window flag + emits the change; `close`/`dismiss` clears it.
- [ ] Overlay mount test: setting the flag mounts `<CommandPalette>`.
- [ ] Regression test failing before the fix, passing after.

## Workflow
- Use `/tdd` — failing test first, then implement.