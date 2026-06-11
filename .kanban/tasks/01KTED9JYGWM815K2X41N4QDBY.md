---
depends_on:
- 01KTED6YMERJHTS7QDSTV5MZYG
- 01KTED7833AJJB5JPTZVNF42HN
- 01KTED7PFKRS6GMAQKVDCQA07V
- 01KTED80H7GNF6YJJTE8MQP7CQ
- 01KTED8MS8917AJCDAVHKSZHK7
- 01KTED8XDX4728QR4WT9EZ0WRF
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9b80
project: ui-command-cleanup
title: Card I — Delete STATIC_GLOBAL_COMMANDS + buildAiCommands dup; remove dead CommandDef.execute fast-path
---
## What
FINAL REMOVAL once no scope-level command executes remain (all prior cards done).

In `apps/kanban-app/ui/src/components/app-shell.tsx`:
- Delete `STATIC_GLOBAL_COMMANDS`: app.command, app.palette, app.undo/redo/dismiss/search/help/quit, settings.keymap.{vim,cua,emacs}, app.resetWindows, file.{newBoard,openBoard,closeBoard}, window.new, app.about — all pure metadata already duplicated in `builtin/plugins/app-shell-commands`, `builtin/plugins/file-commands`, and the window/ui plugins. Verify each id has a plugin equivalent BEFORE deleting; any without one gets added to the appropriate plugin in this card.
- Delete `buildAiCommands` — duplicates `builtin/plugins/ai-commands`. The `ai.cancel` availability gate that `buildAiCommands` computed from `aiStreaming()` must be re-expressed: either as a plugin `available` callback or kept frontend-side per the ai/commands.ts note — preserve the behavior, delete the duplicate definition.

In `apps/kanban-app/ui/src/lib/command-scope.tsx`:
- ~~Remove the now-dead `resolveCommand` execute fast-path and the `CommandDef.execute` field~~ **AMENDED (2026-06-11, Card I implementation): KEPT BY DESIGN.** Legitimate scope-local executes remain in production that the id-keyed webview command bus cannot express: the jump overlay's positional `app.dismiss` shadow (`jump-to-overlay.tsx` — must intercept dismiss only while its scope is innermost, not app-wide for the id), the perspective tab's per-instance scoped rename (`perspective-tab-bar.tsx` — one closure per tab over that tab's state), `ui.entity.startRename`'s window-layer fallback (`buildDynamicGlobalCommands`), and dialog-cancel-style Escape shadows. The bus is a module-level singleton keyed by id alone, so a handler claims the id app-wide — wrong semantics for positional shadows. The decision is documented on `CommandDef.execute`'s JSDoc and at the fast-path in `useDispatchCommand`. Invariant going forward: catalogue (global) commands never carry `execute`; webview-only global behaviors go on the webview command bus.

KEEP (presentation): `use-command-list.ts`, `command-palette.tsx`, `lib/context-menu.ts`, the KeybindingHandler/executeCommand/menu-command+context-menu listeners, keybindings.ts normalize/createKeyHandler/extractChainBindings/extractKeymapBindings.

## Implementation notes (2026-06-11)
- `app.resetWindows` had NO plugin equivalent and NO backend implementation anywhere (grep: only the static def itself) — dispatching it already errored. Deliberately dropped as dead metadata with NO replacement planned, rather than adding a broken plugin command. (Corrected per review: the historical fix card 01KN2GX9ABPFFAFG536SMWN9MY is CLOSED — the command-cutover deleted the dispatch path it targeted — so nothing live tracks the feature; if Reset Windows should ever return, file a fresh card against the window plugin.)
- ai.* executions moved to webview command-bus handlers registered by `AppShell` (`useAiCommandBusHandlers`); the `ai.cancel` gate reads `aiStreaming()` at dispatch time (idle dispatch is a no-op, exactly as `available: false` produced). Registry-palette gating remains the separate follow-up 01KT7DB01HTR9SNRRG145F009P.
- Key parity moved onto plugin metadata in canonical `normalizeKeyEvent` form (registry keys are matched literally by `extractKeymapBindings`): ai-commands `Mod+J/Mod+I` → `Mod+j/Mod+i` (table hoisted to module-level `AI_COMMANDS` for the new drift guard `ai-plugin-commands-mirror.spatial.node.test.ts`, pinned against `BINDING_TABLES`); file-commands `file.closeBoard` `Mod+W` → `Mod+w`; app-shell-commands `app.undo` gained `emacs: "Ctrl+/"` (previously only on the static def). Rust e2e key expectations updated (builtin_file_commands_e2e, builtin_app_shell_commands_e2e).
- Pre-existing focus-scope.test.tsx failures (9) hit during verification are unrelated (file imports nothing changed behaviorally) and already tracked by 01KTS1C4EX8W6GZYPAYB1T431K.

## Acceptance Criteria
- [x] `STATIC_GLOBAL_COMMANDS` and `buildAiCommands` are deleted from app-shell.tsx; every id they carried is plugin-defined (except `app.resetWindows`, deleted as dead — see notes).
- [x] `CommandDef.execute` field and the `resolveCommand` execute fast-path: KEPT by documented design decision (see amendment above) — no catalogue command carries a client execute.
- [x] `ai.cancel` availability behavior preserved (dispatch-time gate in the bus handler; test green).
- [x] Presentation-layer command reading/dispatch still works end to end.

## Tests
- [x] UI: `app-shell.test.tsx` "builds no client-side global CommandDef list — globals resolve from the catalogue" (RED first, then green).
- [x] UI: `app-shell.ai-commands.test.tsx` rewritten — no ai.* CommandDef in scope, five ids webview-bus handled (mount/unmount), ai.toggle/ai.model route to the module registry with no backend call (replaces the no-execute-field criterion per the amendment).
- [x] UI: AI cancel-availability test still green (idle no-op / streaming cancels / gate re-closes).
- [x] New drift guard: `ai-plugin-commands-mirror.spatial.node.test.ts` pins plugin ai.* keys to the canonical `BINDING_TABLES` form (RED on uppercase, green after canonicalization).
- [x] Scoped vitest green (app-shell ×3 files, keybindings, command-scope ×2, use-dispatch-command, mirrors ×5, command-palette, jump-to-overlay, perspective-tab-bar ×3, ai panel suites, webview-bus + guard); `cargo nextest run -p swissarmyhammer-command-service` 127/127; `npx tsc --noEmit` clean.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.

## Review Findings (2026-06-11 14:40)

### Warnings
- [x] `builtin/plugins/app-shell-commands/commands/app.ts:147` (and `:54`, `:100`, `:162`; `builtin/plugins/file-commands/index.ts:121`) — Incomplete canonicalization sweep. This card's own rationale ("the registry is now the only key source for the webview hotkey path, matched literally") was applied to `file.closeBoard` and the `ai.*` keys, but the same reasoning condemns the remaining non-canonical literals, which `normalizeKeyEvent` can never emit: `app.undo` cua `Mod+Z` (this card edited that exact keys block to add emacs `Ctrl+/`), `app.redo` vim `Ctrl+R`, `app.quit` cua `Mod+Q`, `app.search` cua/emacs `Mod+F`, `file.openBoard` cua `Mod+O`. Behavior parity with HEAD holds — the deleted static defs carried the same dead uppercase forms, and native menu accelerators cover the macOS chords — but the deadness is now structural in the single source of truth, and the drift guard covers only `ai.*`. Suggested fix: follow-up card to canonicalize these keys and extend the `*-plugin-commands-mirror` guard pattern to the app-shell/file bundles.
  **RESOLVED (2026-06-11, per prescription): follow-up card 01KTW3NAXYMT4XKA3QWXQGGHVD filed** ($ui-command-cleanup) with per-key canonical targets from `BINDING_TABLES`, the menu-accelerator-only allowlist question (app.quit / file.openBoard), the app.search emacs `Mod+f` ↔ `nav.right` conflict (pre-existing card 01KMT56FTBAP8PQ4QQND08MP97 — must be resolved deliberately, not by silent lowercasing), the table-hoisting needed for `parseCommandTable`, and the Rust e2e expectations to update red-green.
- [x] `apps/kanban-app/ui/src/components/app-shell.tsx:219` — The comment (and the implementation note above) says `app.resetWindows`' brokenness "is tracked by kanban card 01KN2GX9ABPFFAFG536SMWN9MY", but that card sits in the **done** column — nothing live tracks the removed feature. Either reword to "deliberately dropped, no replacement planned" or open/reopen a tracking card if Reset Windows should ever return. (Verified clean otherwise: no registry registration, menu entry, palette row, keybinding, or Tauri command references the id — only doc comments remain.)
  **RESOLVED (2026-06-11): reworded** — the app-shell.tsx comment now says deliberately dropped with no replacement planned (the historical card is closed; the cutover deleted the dispatch path it targeted; a fresh card is the route if the feature should return), and the implementation note above was corrected to match. Reword chosen over a new tracking card because the feature never worked (it crashed via `app.restart()`), its entire backend path is gone, and no live surface references the id.

### Nits
- [x] Kanban card `01KT7DB01HTR9SNRRG145F009P` ("Gate ai.cancel in the palette…") — its "Current state" section still cites `app-shell.tsx`'s `buildAiCommands(streaming)` `available: streaming` / `resolveCommand` gate as the authoritative frontend gate; after this card that gate is the dispatch-time `aiStreaming()` check in `useAiCommandBusHandlers`. Update the card so its implementer isn't pointed at deleted code.
  **RESOLVED (2026-06-11): card 01KT7DB01HTR9SNRRG145F009P's "Current state" section rewritten** — now describes the bus-based reality (no ai.* CommandDef in any React scope; dispatch-time `aiStreaming()` gate in `useAiCommandBusHandlers`, covered by `app-shell.ai-commands.test.tsx`) and its Acceptance now references that gate instead of the deleted `buildAiCommands`.