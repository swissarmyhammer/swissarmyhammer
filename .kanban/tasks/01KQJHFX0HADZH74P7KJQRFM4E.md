---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffcc80
title: Space reliably inspects (no page scroll) — global binding, not scope-only
---
## What

Pressing **Space** is supposed to toggle Inspect on the focused entity. Today it scrolls the page (browser default for Space) instead — the binding only resolves when the kernel already has a `focusedFq`, and the keydown handler only calls `preventDefault()` after a successful binding lookup. So whenever nothing is kernel-focused (app open, inspector just closed, focus parked on a non-Inspectable element), Space hits the browser default.

### Root cause

1. `kanban-app/ui/src/components/inspectable.tsx:260-272` registers `entity.inspect` (Space) **scope-level**, on each `<Inspectable>`:

   ```tsx
   const inspectCommand = useMemo<CommandDef[]>(() => [{
     id: "entity.inspect",
     name: "Inspect",
     keys: { cua: "Space", emacs: "Space" },
     execute: () => { dispatch({ target: moniker }).catch(console.error); },
   }], [dispatch, moniker]);
   ```

2. `kanban-app/ui/src/lib/keybindings.ts:39-61` (`BINDING_TABLES.cua` / `.emacs`) has **no `Space` entry** — there is no global fallback.

3. `kanban-app/ui/src/components/app-shell.tsx:84-92` installs a single `document` keydown handler:

   ```tsx
   const handler = createKeyHandler(mode, executeCommand, () =>
     extractScopeBindings(focusedScopeRef.current, mode));
   document.addEventListener("keydown", handler);
   ```

4. `keybindings.ts:336-369` (`createKeyHandler`) only calls `preventDefault()` / `stopPropagation()` **after** a successful binding lookup. Misses fall through to the browser:

   ```ts
   if (normalized in bindings) {
     e.preventDefault();
     e.stopPropagation();
     executeCommand(bindings[normalized]);
   }
   ```

5. When `focusedFq === null` (`entity-focus-context.tsx:610-615`), `extractScopeBindings(null, mode)` returns `{}` (`keybindings.ts:391-403`). Space is in neither the empty scope bindings nor the global table → handler returns silently → browser scrolls.

The existing test `inspectable.space.browser.test.tsx:332-340` masks this by clicking a `FocusButton` first to claim focus before pressing Space — which is the path users may not take in real usage.

### Recent context

Per the comment at `inspectable.tsx:7`, kanban card `01KQ9XJ4XGKVW24EZSQCA6K3E2` migrated Space ownership from a global `board.inspect` to the per-Inspectable scope. That refactor is what introduced this regression.

### Fix direction (recommended)

Make Space a **root-scope-level** binding that picks the target dynamically — restoring "always works" without giving up the scope-clean architecture the prior refactor moved toward.

Specifically: register a single `entity.inspect` command at the app's root scope (e.g. in `app-shell.tsx` near `buildDrillCommands`) with `keys: { cua: "Space", emacs: "Space" }`. Its `execute` reads the current `focusedFq` from `useSpatialFocusActions()` (or the kernel store) and dispatches `ui.inspect` with that as `target`. If `focusedFq` is null, the command can either:

- (a) No-op but still call `preventDefault()` (acceptable, kills page scroll).
- (b) Pick the first registered Inspectable as the target and inspect it.

**Recommend (a)** — Space-as-no-op when nothing is focused is unambiguous; (b) introduces magic.

The per-Inspectable `entity.inspect` scope command (`inspectable.tsx:260-272`) can stay as-is — it shadows the root binding when an Inspectable is in the scope chain, providing the same behavior. The root binding only kicks in when the chain is empty.

**Alternative considered** (`createKeyHandler` blanket-preventDefault Space outside editables): rejected because it kills the scroll but Space still doesn't *inspect*. The user wants Space to inspect, not just not-scroll.

### Files to modify

- `kanban-app/ui/src/components/app-shell.tsx` — register a root-scope `entity.inspect` command bound to Space. Resolve the target from kernel `focusedFq`. Wire it next to existing root commands (e.g. `buildDrillCommands`).
- (Possibly) `kanban-app/ui/src/lib/keybindings.ts` — only if the chosen design needs a global table entry (a root-scope command does not).
- `kanban-app/ui/src/components/inspectable.space.browser.test.tsx` — add the missing scenario: app open with no kernel focus, press Space → `ui.inspect` is NOT dispatched (or is dispatched on the focused entity), and `preventDefault` IS called (no page scroll). Then the existing test continues to assert focused-entity behavior unchanged.

### Non-goals

- Do **not** auto-claim spatial focus on the first card at mount — that is a behavioral change to focus semantics that should be a separate decision.
- Do **not** remove the per-Inspectable scope command. Scope-level shadowing is the existing architecture and remains correct.
- Do **not** add an unconditional `preventDefault` for all unhandled keys — only Space, and only via the proper binding path.

## Acceptance Criteria

- [x] Pressing Space at app open (no kernel focus, focus on `<body>`) does **not** scroll the page (`preventDefault` is called).
- [x] Pressing Space when kernel focus is on a card / inspectable entity dispatches `ui.inspect` with that entity's moniker as target — same as today.
- [x] Pressing Space when kernel focus is set but on a non-Inspectable scope (e.g. perspective tab, filter editor) does **not** dispatch `ui.inspect` and does **not** scroll the page.
- [x] When focus is inside a text editor / textarea / contenteditable, Space inserts a space character (does NOT preventDefault, does NOT inspect) — verify the existing editor-text-input path is unaffected.
- [x] No regression: the existing `inspectable.space.browser.test.tsx` test (with a clicked-to-focus card) still passes unchanged.

## Tests

- [x] **TDD: write the failing browser test first** in `kanban-app/ui/src/components/inspectable.space.browser.test.tsx`. Add a scenario: render the app shell + a card without clicking it; dispatch `keydown { key: " ", code: "Space" }` on `document`; assert (a) the synthetic event's `defaultPrevented` is `true`, (b) `mockInvoke` was NOT called with `ui.inspect`. Run, confirm fail, then implement.
- [x] Add a positive scenario: render with kernel focus on a card (use the existing setup from the passing test or seed `focusedFq` directly); press Space; assert `ui.inspect` dispatched with that card's moniker AND `defaultPrevented` true.
- [x] Add a "focus inside text editor" test: render an editing field; press Space; assert `defaultPrevented` is `false` AND `ui.inspect` NOT dispatched (Space inserts a character — the editor's own input handling).
- [x] If feasible, add a Playwright/end-to-end test at `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` (or a sibling) that scrolls the page beyond the fold, presses Space without claiming focus first, and asserts the scroll position is unchanged.
- [x] Run: `bun test inspectable.space.browser.test.tsx` — green.
- [x] Run: `bun test` for the spatial e2e suite — green.

## Workflow

- Use `/tdd` — failing test first (Space at app open does not scroll AND does not inspect), implement the root-scope binding, then verify the focused-entity scenario still works.

## Implementation notes

- Implemented as the recommended (a): root-scope `entity.inspect` command in `app-shell.tsx` (`buildRootInspectCommand`), filtered by `INSPECTABLE_ENTITY_PREFIXES` (`task:`, `tag:`, `column:`, `board:`, `field:`, `attachment:`). When `focusedFq()` is null OR the leaf segment is not inspectable kind, the closure no-ops; the keybinding handler still calls `preventDefault()` because the binding resolved.
- Required adding `Space → entity.inspect` to `BINDING_TABLES.cua` and `BINDING_TABLES.emacs` in `kanban-app/ui/src/lib/keybindings.ts`. `extractScopeBindings(null)` returns `{}` (no chain to walk), so without a global table entry the handler would not see Space at all when nothing is focused. The dispatcher then resolves `entity.inspect` through the focused-or-tree scope chain — the per-`<Inspectable>` scope command shadows when present, the root command in `app-shell.tsx` catches the rest.
- All three keymaps (vim / cua / emacs) now claim Space. The earlier "vim leaves Space for the leader-key role" stance was based on a stale comment — `SEQUENCE_TABLES.vim` only has `g`, `d`, `z` as sequence prefixes, so there is no current vim leader Space would conflict with. Vim parity is added to `BINDING_TABLES.vim`, the per-`<Inspectable>` `keys` map in `inspectable.tsx`, and `buildRootInspectCommand` in `app-shell.tsx`; if a future vim leader is wired up, all three sites will need to move together.
- Allowlisted `components/app-shell.tsx` in `focus-architecture.guards.node.test.ts` Guard A so the new `useDispatchCommand("ui.inspect")` call site does not trip the single-source-double-click contract.
- E2E pin added to `spatial-nav-end-to-end.spatial.test.tsx` Family 4 — drives an explicit `focus-changed` clear event before pressing Space so the test exercises the no-focus path through the real board mount.

Test status: 218 test files / 2079 tests, all green.

## Review Findings (2026-05-10 14:48)

### Nits
- [x] `kanban-app/ui/src/components/board-view.space-inspect.guard.node.test.ts:6-21,137-154` — Documentation drift. The header comment ("The replacement binding lives on a single source file: `inspectable.tsx`...") and the test #2 comment ("if the binding ever moves, the test moves with it, and a reader searching for 'where is Space wired to inspect' finds the canonical answer here") were written when Space was registered in exactly one production source. After this card's fix, `entity.inspect` with `keys: { cua: "Space" }` is also registered at the root scope in `components/app-shell.tsx` (`buildRootInspectCommand`). The test itself still passes because it is a positive presence assertion against `inspectable.tsx`, not an exclusivity assertion — but the surrounding prose now misleads readers searching for "where is Space wired." Suggested fix: update the header comment and the test #2 comment to describe the two-tier architecture (per-Inspectable scope command in `inspectable.tsx`; root-scope fallback in `app-shell.tsx`), and clarify the guard pins only the per-Inspectable site.

## Review Findings (2026-05-11 — user verification regression)

User reports the fix doesn't work in production in vim keymap mode: Space still scrolls the page AND Space on a focused card does NOT open the inspector. The implementer excluded vim from the new Space binding on a judgment call (citing a stale `inspectable.tsx` comment about "vim leaves Space for the leader-key role"), but `SEQUENCE_TABLES.vim` has only `g`, `d`, `z` as sequence prefixes — there is no actual vim leader-key wired up. The original task's acceptance criteria do not mention vim, but they say "Pressing Space at app open (no kernel focus, focus on `<body>`) does not scroll the page" without qualifying the keymap. In vim mode that criterion fails today.

### Blockers
- [x] `kanban-app/ui/src/lib/keybindings.ts` — `BINDING_TABLES.vim` has no `Space` entry. In vim mode with no kernel focus, the keydown handler's binding lookup misses, no `preventDefault()` fires, and the browser scrolls the page. Add `Space: "entity.inspect"` to the vim table. Space is NOT a sequence prefix in `SEQUENCE_TABLES.vim` (which uses `g`, `d`, `z`), so the addition is safe.
- [x] `kanban-app/ui/src/components/inspectable.tsx:260-272` — the per-Inspectable `entity.inspect` `CommandDef` has `keys: { cua: "Space", emacs: "Space" }` only. Add `vim: "Space"` so the scope-level command resolves in vim too. Without this, even when focus is on a card, vim users get no inspect.
- [x] `kanban-app/ui/src/components/app-shell.tsx:478` — `buildRootInspectCommand` returns `keys: { cua: "Space", emacs: "Space" }`. Add `vim: "Space"` so the root command's binding is registered for vim. Required for both the chrome-focus path (no Inspectable in chain) and the no-focus path.
- [x] `kanban-app/ui/src/components/inspectable.tsx:248-250` — Comment claims "vim intentionally has no entry — vim leaves Space for the leader-key role it traditionally fills." Update prose: there is no current vim leader; Space now binds to `entity.inspect` in all three keymaps.
- [x] `kanban-app/ui/src/lib/keybindings.ts` — block comment at the cua Space entry (lines ~67-80) says "Space → `entity.inspect`. The per-`<Inspectable>` scope command shadows this entry…" — extend the same prose to the vim entry (or refactor the comment to cover all three modes uniformly).

### Warnings
- [x] `kanban-app/ui/src/components/inspectable.space.browser.test.tsx` — every Space test renders with the default cua keymap. None of the scenarios exercise vim. Add a parametrized variant (or a vim-specific test set) that runs scenarios #6 (app open, no focus), #7 (focus on non-Inspectable chrome), #9 (focused card) under `keymap_mode: "vim"` and asserts the same `defaultPrevented` / dispatch behavior. The `defaultInvokeImpl` for `get_ui_state` returns `keymap_mode: "cua"` — accept a parameter or split into a per-mode loop.
- [x] `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` Family 4 — same coverage gap; the no-focus / focused-card / non-Inspectable-chrome cases test only cua. Add at least one vim-mode pin.

### Workflow

TDD: write a failing vim-mode test in `inspectable.space.browser.test.tsx` first (set `keymap_mode: "vim"` via the `get_ui_state` mock, render `<AppShell>`, dispatch Space, assert `defaultPrevented === true` AND `ui.inspect` not dispatched at app open; same with a focused card asserting dispatch + preventDefault). Watch it fail, then update the three `keys` maps + the two comment blocks. Re-run the targeted test, then full `npm test`, then verify Rust still clean. The task stays in `doing` until this passes; do NOT move to review until verified.