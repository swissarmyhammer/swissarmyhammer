---
assignees:
- claude-code
position_column: todo
position_ordinal: c680
title: 'Bug: Cannot launch command palette — not in menu, not by hotkey'
---
## What
Reported by user: the command palette cannot be opened — neither from the native menu nor by a keyboard shortcut. (User pressed a hotkey and nothing happened.)

## It is NOT a window — it's an overlay React component
`<CommandPalette>` is rendered in `app-shell.tsx:818-826` as `<FocusLayer name="palette"><CommandPalette/></FocusLayer>`, **portaled to `document.body`** (`createPortal`, see comment app-shell.tsx:741-746). It is conditionally mounted only when `paletteOpen` is true. `paletteOpen = winState?.palette_open` (app-shell.tsx:671) — the **per-window backend UIState flag**. So the open path is: command → backend sets `palette_open=true` → `UIStateChange::PaletteOpen` → `ui-state-context` updates `windows[label].palette_open` → AppShell re-renders → overlay mounts. No OS window involved.

## Root cause (investigated): three-way divergence in the open path + no menu entry
There are **three** palette-open command IDs, all wired to backend handlers, but the frontend keybindings only listen for one of them:

| Command ID | Backend handler (`commands/mod.rs`) | Declared keys | Frontend listens? |
|---|---|---|---|
| `ui.palette.open` | `PaletteOpenCmd` (mod.rs:150) | `ui.yaml`: cua **Mod+K**, vim `:` | ❌ NOT in `BINDING_TABLES`, NOT in any frontend scope |
| `app.command` | `CommandPaletteCmd` (mod.rs:310) | `app.yaml`: vim `:`, cua/emacs **Mod+Shift+P** | ✅ vim `:` → `app.command` |
| `app.palette` | `CommandPaletteCmd` alias (mod.rs:315) | **Mod+Shift+P** all modes | ✅ `Mod+Shift+P` → `app.palette` |

- Frontend static `BINDING_TABLES` (`apps/kanban-app/ui/src/lib/keybindings.ts:27-140`) only bind `Mod+Shift+P → app.palette` (all modes) and vim `:` → `app.command`.
- The frontend global command set (`STATIC_GLOBAL_COMMANDS`, app-shell.tsx:170-180) contains `app.command` and `app.palette` — but **NOT `ui.palette.open`**. So `extractScopeBindings` never surfaces `ui.palette.open`, and `Mod+K` is not in `BINDING_TABLES` → **`Mod+K` resolves to nothing in the live app.**
- No palette command (`ui.palette.open` / `app.command` / `app.palette`) carries a `menu:` placement → **no native menu affordance at all** (matches "not on the menu").

**Conclusion:** the registry/menu world advertises `ui.palette.open` / `Mod+K`, but the running frontend only opens the palette via **`Mod+Shift+P`** (or vim `:`). The user almost certainly pressed the advertised `Mod+K`, which is dead on the frontend.

## First implementer step — bisect A vs B
1. **A (most likely):** `Mod+Shift+P` DOES open the palette → the bug is the keybinding/menu divergence. Fix by consolidating to ONE palette command (prefer `ui.palette.open`), wiring its key into `BINDING_TABLES` / the frontend global scope, deleting the `app.command`/`app.palette` duplication, and adding a `menu:` placement (View or App) so there's a clickable affordance.
2. **B (deeper):** if `Mod+Shift+P` ALSO does nothing, the failure is downstream — `palette_open` not propagating (`ui-state-context`), the `UIStateChange::PaletteOpen` envelope not unwrapping (cf. the `{ok,change}` envelope issue in recent commit af9e6e965), or the `<FocusLayer parentLayerFq>` mount being rejected. Capture `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` while pressing the key.

## Acceptance Criteria
- [ ] A single documented shortcut opens the palette in all keymap modes, and it matches what the registry/menu advertise (no Mod+K vs Mod+Shift+P divergence).
- [ ] There is a discoverable native-menu affordance that opens the palette.
- [ ] The three-way `ui.palette.open` / `app.command` / `app.palette` duplication is reconciled to a single source of truth (or intentional aliases that all share one binding wired into the frontend).
- [ ] Root cause confirmed as A or B (see bisect step) and documented.

## Tests
- [ ] Frontend: extend `apps/kanban-app/ui/src/lib/keybindings.test.ts` so the canonical palette shortcut resolves to the canonical palette command under the real board scope chain.
- [ ] A `menu.rs` test asserting the palette command collects into its intended submenu key (once a `menu:` placement is added).
- [ ] If B: a `ui-state-context` test that a `PaletteOpen` change flips `windows[label].palette_open` and mounts the overlay.
- [ ] Regression test failing before the fix, passing after.

## Workflow
- Use `/tdd` — failing test first, then fix.