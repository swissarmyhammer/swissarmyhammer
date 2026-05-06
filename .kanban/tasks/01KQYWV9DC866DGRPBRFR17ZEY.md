---
assignees:
- claude-code
depends_on:
- 01KQYWM5BHFRPCRD70GF8YRCGY
- 01KQYWSW6NFHCS53JT9Y8NYK47
position_column: todo
position_ordinal: e380
project: spatial-nav
title: Wire nav.jump command — YAML, keybindings, menu, palette, app-shell state
---
## What

Add the `nav.jump` command and connect every entry point to a single `<JumpToOverlay>` instance mounted in `app-shell.tsx`.

Touch points:

1. **YAML** — append to `swissarmyhammer-commands/builtin/commands/nav.yaml` (created by the prior task):

   ```yaml
   - id: nav.jump
     name: Jump To
     undoable: false
     keys:
       vim: s
       cua: Mod+G
       emacs: Mod+G
     menu:
       path: [Navigation]
       group: 3
       order: 0
   ```

   Group 3 places it after up/down/left/right (group 0), first/last (group 1), and drillIn/drillOut (group 2) with a separator.

2. **Keybindings** — `kanban-app/ui/src/lib/keybindings.ts`:
   - Add `s: "nav.jump"` to the `vim` `BINDING_TABLES` block (around line 97).
   - Add `"Mod+g": "nav.jump"` to the `cua` block (around line 108).
   - Add `"Mod+g": "nav.jump"` to the `emacs` block (around line 131).
   - Verify vim's `s` does not conflict with any existing chord prefix in `SEQUENCE_TABLES.vim` (currently `g`, `d`, `z` — `s` is free).

3. **App-shell wiring** — `kanban-app/ui/src/components/app-shell.tsx`:
   - Add `const [jumpOpen, setJumpOpen] = useState(false);` to the AppShell component.
   - Render `<JumpToOverlay open={jumpOpen} onClose={() => setJumpOpen(false)} />` near the existing modal mounts (e.g., command palette).
   - In the `globalCommands` memo, append a `nav.jump` CommandDef whose `execute: async () => setJumpOpen(true)`. Use the same memo pattern as the other nav commands.

4. **Menu wiring** — `kanban-app/src/menu.rs`:
   - The Navigation submenu was added in the prior task; `nav.jump` will appear automatically because it has `menu.path: [Navigation]`.
   - Confirm the menu click for `nav.jump` is dispatched through the same Tauri menu-event path that existing commands use, ending in the React `execute` closure (no extra plumbing needed).

5. **Palette discoverability** — automatic once the YAML stub has `name: "Jump To"`. Verify by opening the palette and typing "jump".

## Acceptance Criteria

- [ ] Pressing `s` in vim mode opens the Jump-To overlay.
- [ ] Pressing `Cmd+G` (mac) / `Ctrl+G` (linux/win) in cua or emacs mode opens the Jump-To overlay.
- [ ] Selecting `Navigation > Jump To` from the native menu opens the overlay.
- [ ] Typing "jump" in the command palette (`Mod+Shift+P`) shows the `Jump To` command; selecting it opens the overlay.
- [ ] All four entry points open the same overlay instance — the state lives in `app-shell.tsx`, not duplicated.

## Tests

- [ ] Add to `kanban-app/ui/src/components/app-shell.nav-commands.test.tsx` (created in the YAML-promotion task): assert `globalCommands` includes `nav.jump` and its `execute` flips the AppShell's `jumpOpen` state to `true`. Mount AppShell, simulate a `vim`-mode `s` keystroke, assert the overlay is rendered.
- [ ] Test command: `cd kanban-app/ui && pnpm test app-shell.nav-commands jump-to-overlay` — passes.
- [ ] Manual sanity check via the full registry test in the YAML-promotion task: all 9 nav commands (the original 8 + `nav.jump`) load with menu placement.

## Workflow

- Use `/tdd` — extend the existing app-shell test with the `s` keystroke → overlay assertion (will fail because the binding doesn't exist); then add YAML, keybinding, and state; re-run. #nav-jump