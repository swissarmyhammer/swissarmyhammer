---
assignees:
- claude-code
position_column: todo
position_ordinal: ef80
project: ui-command-cleanup
title: Canonicalize remaining app-shell/file plugin key literals; extend the mirror drift guards to those bundles
---
## What
Follow-up prescribed by the Card I review (01KTED9JYGWM815K2X41N4QDBY, warning W1): finish the key-canonicalization sweep that Card I applied to `file.closeBoard` and the `ai.*` keys, and extend the `*-plugin-commands-mirror` drift-guard pattern to the app-shell and file bundles.

Since Card I deleted `app-shell.tsx`'s static scope defs, the plugin registry metadata is the ONLY key source for the webview hotkey path: `extractKeymapBindings` matches the declared string LITERALLY against `normalizeKeyEvent` output, which emits lowercase letters for unshifted chords. The following declared keys are therefore structurally unreachable from a real keydown (today the macOS chords ride native menu accelerators, which parse letters case-insensitively):

- `builtin/plugins/app-shell-commands/commands/app.ts` — `app.quit` cua `Mod+Q`; `app.search` cua `Mod+F` + emacs `Mod+F`; `app.undo` cua `Mod+Z`; `app.redo` vim `Ctrl+R`
- `builtin/plugins/file-commands/index.ts` — `file.openBoard` cua `Mod+O`

## Per-key prescriptions (canonical reference: `BINDING_TABLES` in `apps/kanban-app/ui/src/lib/keybindings.ts`)
- `app.undo` cua `Mod+Z` → `Mod+z` (BINDING_TABLES.cua agrees: `Mod+z` → app.undo). The emacs `Ctrl+/` is already canonical.
- `app.redo` vim `Ctrl+R` → `Mod+r` per BINDING_TABLES.vim (`Mod+r` → app.redo; non-Mac Ctrl+R normalizes to Mod+r anyway — on Mac this means Cmd+R, since Ctrl stays distinct there).
- `app.quit` cua `Mod+Q` → `Mod+q`. No BINDING_TABLES entry exists (quit rides the native menu accelerator); lowercasing keeps the accelerator (case-insensitive parse) AND makes the chord live in the webview on non-Mac — the `file.closeBoard` precedent from Card I. Alternatively declare it menu-accelerator-only explicitly; either way the new guard must encode the decision.
- `file.openBoard` cua `Mod+O` → `Mod+o` (same reasoning as app.quit).
- `app.search` cua `Mod+F` → `Mod+f` (BINDING_TABLES.cua agrees). emacs `Mod+F`: CAUTION — do NOT blindly lowercase. BINDING_TABLES.emacs binds `Mod+f` → `nav.right` (the non-Mac normalization of emacs forward-char Ctrl+f; `nav-commands` declares emacs `Ctrl+f`). Canonicalizing to `Mod+f` would make the registry global table claim Mod+f for app.search, changing non-Mac emacs Ctrl+F from navigate-right to Find and re-opening the first-id-wins nondeterminism class (card 01KTQ6QZNB3VN4MAND7VPASM21). This is the pre-existing conflict card 01KMT56FTBAP8PQ4QQND08MP97 documents — resolve it deliberately here (likely: drop the emacs key from app.search, leaving emacs Find to the palette) rather than as a lowercasing side effect.

## Guard extension (mirror the ai-commands pattern)
- Hoist the inline command arrays into module-level `const` data tables parseable by `apps/kanban-app/ui/src/test/plugin-command-table.ts::parseCommandTable` (the way Card I hoisted `AI_COMMANDS`): e.g. `APP_COMMANDS` in `app-shell-commands/commands/app.ts` and `FILE_COMMANDS` in `file-commands/index.ts`. (`parseCommandTable` anchors on `NAME ... = [ ... \n];` — a `return [...]` inside a function does not parse.)
- Add `app-shell-plugin-commands-mirror.spatial.node.test.ts` and `file-plugin-commands-mirror.spatial.node.test.ts` next to `ai-plugin-commands-mirror.spatial.node.test.ts`, pinning declared keys to `BINDING_TABLES`. Two deltas from the ai guard to design for: (1) `app.search` has TWO vim keys in BINDING_TABLES (`/` and `Mod+f`) — assert membership, not single-key equality; (2) menu-accelerator-only keys with no BINDING_TABLES entry (app.quit, file.openBoard, file.newBoard `Mod+Shift+B`) need an explicit, commented allowlist so the guard still fails on a NEW unexplained key.

## Keep test expectations in sync (red-green)
- `crates/swissarmyhammer-command-service/tests/integration/builtin_app_shell_commands_e2e.rs` pins the current uppercase forms for app.quit / app.search / app.undo / app.redo; `builtin_file_commands_e2e.rs` pins `Mod+O` for file.openBoard. Update the expectations FIRST, watch them fail, then change the plugin metadata.
- Also re-check `apps/kanban-app/ui/src/test/mock-command-list.ts` mirrors and the keybindings comment blocks for stale uppercase references.

## Acceptance Criteria
- [ ] No plugin-declared key literal in app-shell-commands / file-commands is unreachable by `normalizeKeyEvent` unless explicitly allowlisted as menu-accelerator-only with a comment.
- [ ] The app.search emacs conflict (01KMT56FTBAP8PQ4QQND08MP97) is resolved deliberately, not by silent lowercasing.
- [ ] New mirror guards fail RED on an uppercase regression (verify by temporarily re-uppercasing one key).
- [ ] `cargo nextest run -p swissarmyhammer-command-service` green; scoped vitest (new guards + keybindings + app-shell suites) green; `npx tsc --noEmit` clean.

## Workflow
- `/tdd` — failing expectations first, then metadata changes.