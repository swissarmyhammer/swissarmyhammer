---
assignees:
- wballard
position_column: todo
position_ordinal: b380
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

- [ ] Pressing Space at app open (no kernel focus, focus on `<body>`) does **not** scroll the page (`preventDefault` is called).
- [ ] Pressing Space when kernel focus is on a card / inspectable entity dispatches `ui.inspect` with that entity's moniker as target — same as today.
- [ ] Pressing Space when kernel focus is set but on a non-Inspectable scope (e.g. perspective tab, filter editor) does **not** dispatch `ui.inspect` and does **not** scroll the page.
- [ ] When focus is inside a text editor / textarea / contenteditable, Space inserts a space character (does NOT preventDefault, does NOT inspect) — verify the existing editor-text-input path is unaffected.
- [ ] No regression: the existing `inspectable.space.browser.test.tsx` test (with a clicked-to-focus card) still passes unchanged.

## Tests

- [ ] **TDD: write the failing browser test first** in `kanban-app/ui/src/components/inspectable.space.browser.test.tsx`. Add a scenario: render the app shell + a card without clicking it; dispatch `keydown { key: " ", code: "Space" }` on `document`; assert (a) the synthetic event's `defaultPrevented` is `true`, (b) `mockInvoke` was NOT called with `ui.inspect`. Run, confirm fail, then implement.
- [ ] Add a positive scenario: render with kernel focus on a card (use the existing setup from the passing test or seed `focusedFq` directly); press Space; assert `ui.inspect` dispatched with that card's moniker AND `defaultPrevented` true.
- [ ] Add a "focus inside text editor" test: render an editing field; press Space; assert `defaultPrevented` is `false` AND `ui.inspect` NOT dispatched (Space inserts a character — the editor's own input handling).
- [ ] If feasible, add a Playwright/end-to-end test at `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` (or a sibling) that scrolls the page beyond the fold, presses Space without claiming focus first, and asserts the scroll position is unchanged.
- [ ] Run: `bun test inspectable.space.browser.test.tsx` — green.
- [ ] Run: `bun test` for the spatial e2e suite — green.

## Workflow

- Use `/tdd` — failing test first (Space at app open does not scroll AND does not inspect), implement the root-scope binding, then verify the focused-entity scenario still works.