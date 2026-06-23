---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvrxb8s7a8nga1x3wezxwqp5
  text: |-
    Picked up. Research done. Plan:

    KEY CHANGES (app.ts):
    - app.undo cua Mod+Z -> Mod+z
    - app.redo vim Ctrl+R -> Mod+r
    - app.quit cua Mod+Q -> Mod+q (menu-accelerator-only; no BINDING_TABLES entry)
    - app.search cua Mod+F -> Mod+f; DROP emacs key entirely (emacs Mod+f is bound to nav.right in BINDING_TABLES.emacs; lowercasing would hijack emacs Ctrl+F nav-right -> Find and reopen first-id-wins nondeterminism per 01KTQ6QZNB3VN4MAND7VPASM21/01KMT56FTBAP8PQ4QQND08MP97). Leave emacs Find to the palette.

    KEY CHANGE (file-commands/index.ts):
    - file.openBoard cua Mod+O -> Mod+o (menu-accelerator-only; no BINDING_TABLES entry)

    HOISTING: APP_COMMANDS in app.ts (move array out of appCommands() into module const param-table; appCommands returns it), FILE_COMMANDS in file-commands/index.ts.

    GUARDS: app-shell-plugin-commands-mirror + file-plugin-commands-mirror spatial.node.test.ts. Deltas: app.search has 2 vim keys in BINDING_TABLES (/ and Mod+f) -> assert membership; menu-accelerator-only keys (app.quit, file.openBoard, file.newBoard Mod+Shift+B) -> commented allowlist.

    TDD: update Rust e2e expectations (builtin_app_shell_commands_e2e.rs app.quit/search/undo/redo; builtin_file_commands_e2e.rs file.openBoard) to canonical forms FIRST, watch fail, then change metadata.

    mock-command-list.ts derives app.* keys from BINDING_TABLES (no hardcoded uppercase) so it auto-tracks. Grep found no stale uppercase literals in ui/src except unrelated test comments.
  timestamp: 2026-06-23T00:16:10.535118+00:00
- actor: claude-code
  id: 01kvry5h0bmt00tc7q3gjfkmgq
  text: |-
    DONE — all acceptance gates green. Moving to review.

    KEY CHANGES (builtin/plugins/app-shell-commands/commands/app.ts, hoisted to module-level const APP_COMMANDS):
    - app.undo cua Mod+Z -> Mod+z (BINDING_TABLES.cua member)
    - app.redo vim Ctrl+R -> Mod+r (BINDING_TABLES.vim member)
    - app.quit cua Mod+Q -> Mod+q (menu-accelerator-only; allowlisted)
    - app.search cua Mod+F -> Mod+f (BINDING_TABLES.cua member); emacs key DROPPED

    EMACS-CONFLICT RESOLUTION: app.search emacs Mod+F dropped, NOT lowercased. BINDING_TABLES.emacs binds Mod+f -> nav.right; lowercasing would hijack emacs Ctrl+F nav-right -> Find and reopen first-id-wins nondeterminism (01KTQ6QZNB3VN4MAND7VPASM21 / 01KMT56FTBAP8PQ4QQND08MP97). Emacs Find left to the palette. Documented in code comment + both Rust e2e doc-comment + the new guard's allowlist logic (a re-added emacs app.search key fails: no BINDING_TABLES entry + not allowlisted).

    KEY CHANGE (builtin/plugins/file-commands/index.ts, hoisted to module-level const FILE_COMMANDS):
    - file.openBoard cua Mod+O -> Mod+o (menu-accelerator-only; allowlisted)
    Fixed dispatch path to windowD.window.window.board.<verb> (server.tool.noun.verb) after hoist.

    HOISTING: both arrays moved to module-level const so parseCommandTable (anchors on NAME...=[...\n];) parses them. Executes close over module-level dispatch holders (appD/storeD/uiStateD; windowD) set synchronously before registerCommands. Per-isolate, single-instance — safe.

    GUARDS (NEW): apps/kanban-app/ui/src/test/{app-shell,file}-plugin-commands-mirror.spatial.node.test.ts. Each: (1) PINNED check — declared key must be a MEMBER of BINDING_TABLES keys bound to that id in that mode (handles app.search's two vim keys / and Mod+f); (2) ALLOWLISTED check — keys with no BINDING_TABLES entry must match a COMMENTED MENU_ACCELERATOR_OR_PALETTE / MENU_ACCELERATOR_ONLY allowlist's EXACT value (app.quit, app.help, app.command, app.palette, app.undo emacs Ctrl+/; file.newBoard Mod+Shift+B, file.openBoard Mod+o). Exact-value allowlist => uppercase regression of an allowlisted letter chord ALSO fails. New unexplained key => fails.

    RED-GUARD PROOF: temporarily re-uppercased app.undo cua Mod+z->Mod+Z and file.openBoard Mod+o->Mod+O; both new guards FAILED:
    - app guard: "keys.cua for app.undo: plugin \"Mod+Z\" is not a BINDING_TABLES member of {\"Mod+z\"}"
    - file guard: "keys.cua for file.openBoard: plugin \"Mod+O\" has no BINDING_TABLES entry and is not the allowlisted \"Mod+o\""
    Then reverted; both green again.

    TDD: Rust e2e expectations updated FIRST (watched RED: app.quit Mod+Q vs Mod+q etc., file.openBoard Mod+O vs Mod+o), then plugin metadata changed -> GREEN.

    mock-command-list.ts: app.* keys derive from BINDING_TABLES, auto-tracking; no edit needed. No stale uppercase literals in ui/src production/test code (only unrelated cm-editor Mod+Z comments).

    VERIFICATION (fresh):
    - cargo nextest run -p swissarmyhammer-command-service: 168 passed, 0 skipped
    - npx tsc --noEmit: clean (exit 0)
    - npx vitest run (7 files: both new guards, keybindings, ai guard, app-shell.test + ai-commands + nav-commands): 138 passed
    - cargo fmt: clean

    OUT OF SCOPE (left untouched): crates/swissarmyhammer-kanban/src/commands_core/registry.rs has a #[cfg(test)] synthetic YAML parser fixture with Mod+Q/Mod+Z — it tests the YAML loader, is consumer-agnostic, and is NOT the app-shell/file plugin metadata; changing it would be scope creep.

    double-check agent (adversarial): PASS — confirmed emacs drop is clean, holder pattern safe, parser non-vacuous, guards RED-able, no other test pins old forms, and the NATIVE menu accelerator path (apps/kanban-app/src/menu.rs::resolve_accelerator) derives accelerators from keys metadata at runtime so lowercasing propagates correctly (muda parses letters case-insensitively).
  timestamp: 2026-06-23T00:30:30.923359+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffdf80
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