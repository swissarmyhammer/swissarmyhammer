---
depends_on:
- 01KTED5F8DQ2XH5BB0WK1MRR3P
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9680
project: ui-command-cleanup
title: Card D — Move field.edit/editEnter + pressable.activate/activateSpace to plugins + handler bus
---
## What
Move the field-edit and pressable-activate command DEFINITIONS out of React and into plugins, with their webview behaviors routed through the handler bus (Card B).

- `apps/kanban-app/ui/src/components/fields/field.tsx`: `field.edit` / `field.editEnter` (enter field edit mode) — WEBVIEW behavior.
- `apps/kanban-app/ui/src/components/pressable.tsx`: `usePressCommands` defining `pressable.activate` / `pressable.activateSpace` (call local `onPress`) — WEBVIEW behavior.

Approach:
- Define `field.edit`, `field.editEnter`, `pressable.activate`, `pressable.activateSpace` in a plugin — likely the existing `builtin/plugins/ui-commands/index.ts` (these are generic UI-surface commands) — with id/name/keys/scope; no backend op, marked "handled in webview".
- In field.tsx and pressable.tsx, replace the client-side command defs / `usePressCommands` def-building with `registerWebviewCommandHandler(id, handler)` registrations keyed by the plugin ids. The components keep owning the edit-mode and onPress logic as handlers.
- Preserve the activation contracts (Space/Enter) that existing tests pin.

## Acceptance Criteria
- [x] `field.edit`, `field.editEnter`, `pressable.activate`, `pressable.activateSpace` are defined by a plugin; field.tsx and pressable.tsx no longer DEFINE them.
- [x] field.tsx / pressable.tsx register webview handlers keyed by those ids; dispatching each runs the original behavior via the bus.
- [x] Space/Enter activation and field-edit-enter behavior unchanged.
- [x] GUARD (presentation-only invariant): the edit-mode toggle and `onPress` handlers are pure presentation (local state / DOM focus only). field.tsx and pressable.tsx must NOT import `@/lib/mcp-transport`; any durable effect routes via `useDispatchCommand`. `webview-command-bus.guard.node.test.ts` stays green.

## Tests
- [x] UI: extend `apps/kanban-app/ui/src/components/fields/field.enter-edit.browser.test.tsx` to assert `field.edit`/`field.editEnter` dispatch through the bus into edit mode.
- [x] UI: extend `apps/kanban-app/ui/src/components/pressable.test.tsx` to assert `pressable.activate`/`pressable.activateSpace` invoke `onPress` via the bus.
- [x] Plugin e2e: the chosen plugin registers the four ids with expected metadata.
- [x] `webview-command-bus.guard.node.test.ts` green with field.tsx and pressable.tsx as registration sites.
- [x] Relevant vitest files green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.

## Implementation notes (done)
- The four ids live in `builtin/plugins/ui-commands/index.ts::UI_SURFACE_COMMANDS` (data table, mirrors `grid-commands`' `GRID_COMMANDS`), registered with `undoable:false`, inert host execute, and per-surface scope gates `["ui:field"]` / `["ui:pressable"]`.
- Fields/pressables are many-instance surfaces with dynamic monikers, so each component mounts a constant MARKER moniker (`CommandScopeProvider` `ui:field` / `ui:pressable` directly above its `<FocusScope>`) for literal scope matching, and registers its live behavior on the webview bus WHILE SPATIAL FOCUS IS WITHIN ITS SUBTREE via the shared hook `src/lib/use-focused-webview-command-handlers.ts` — so a dispatched id always reaches the containing instance's closure (subtrees of distinct instances are disjoint).
- Keymap precedence: replaced app-shell's two-spread merge (scope defs always beat scoped registry) with a depth-interleaved single walk `extractChainBindings` (keybindings.ts) — innermost wins across BOTH sources; component defs win at equal depth. Required so a focused Pressable's registry Space beats the enclosing `<Inspectable>`'s `entity.inspect` def, exactly as the retired leaf defs did.
- Mirror drift guard: `UI_SURFACE_PLUGIN_COMMANDS` in mock-command-list.ts + `ui-surface-plugin-commands-mirror.spatial.node.test.ts` (parses the plugin source).
- Rust: builtin_ui_commands_e2e now asserts 14 registrations + metadata + inert dispatch for the four ids; full_baseline_e2e id set is 92.
- Pre-existing browser-suite failures (verified by stash-baseline, NOT from this card): focus-scope.test, entity-inspector.test, board-view.enter-drill-in, entity-focus.kernel-projection, inspectable.space, entity-inspector.field-vertical-nav, inspector.* (boundary-nav/close-restores/repeat-open/auto-focus), column-view.test/virtualized-nav, perspective-tab-bar.filter-migration, grid-view.cursor-ring, grid-empty-state, mention-view, path-monikers, entity-card.test, attachment-display, entity-inspector.field-enter-drill (2), perspective-context, plus file-level `SERIALIZE_TO_IPC_FN` import errors (avatar, board-selector, select/attachment editors, avatar-display).

## Review Findings (2026-06-11 08:58)

### Blockers
- [x] `apps/kanban-app/ui/src/lib/use-focused-webview-command-handlers.ts:90` / `apps/kanban-app/ui/src/components/fields/field.tsx:646` — Keyboard path into multi-value field editors regressed. The keymap layer binds Enter / vim `i` to `field.edit`/`field.editEnter` whenever the `ui:field` marker is ANYWHERE in the focused chain (`extractChainBindings`), but the bus handler registers only while the field zone is the STRICT direct focus (`useOptionalIsDirectFocus` returns `current === moniker`). When a pill inside the field is focused — the very state `field.edit`'s own drill-in puts the user in — the key still resolves to the plugin id, the bus slot is empty, and dispatch lands on the plugin's inert host execute: a silent no-op that still `preventDefault()`s. Fix: gate the bus registration on focus-WITHIN the field zone's subtree (matching the marker-in-chain gate the keymap uses).

### Warnings
- [x] `apps/kanban-app/ui/src/lib/keybindings.ts:553` / `:650` — `extractScopeBindings` and `extractScopedRegistryBindings` have zero production callers after app-shell moved to `extractChainBindings` (verified: every non-test reference is a doc comment). Both are exported wrappers kept alive only by tests. Fold their test coverage onto `extractChainBindings` (or mark them explicitly test-only) so the public surface matches the production path.

### Nits
- [x] `apps/kanban-app/ui/src/lib/keybindings.ts:60,97` and `apps/kanban-app/ui/src/components/app-shell.tsx:336,398,449` — stale doc comments still describe `extractScopeBindings` as the live keystroke resolution path; production now resolves through `extractChainBindings`.
- [x] `ARCHITECTURE.md` — the webview command bus, the `ui:field`/`ui:pressable` marker-moniker scope-gating pattern, and the depth-interleaved keymap walk are now load-bearing structural seams (Cards B–D) but are absent from the architecture doc; recommend an update.

## Review Findings Resolution (2026-06-11)

- BLOCKER fixed (strict red-first TDD): new test `enter_on_pill_inside_tags_field_opens_the_field_editor` in `field.enter-edit.browser.test.tsx` renders a production-shaped tags field (badge-list, multi-select editor, real MentionView pills), focuses a pill, presses Enter. RED before fix: "field.edit must have a live bus handler while a pill inside the field is focused: expected false to be true" (the dead-key state). GREEN after fix: 6/6 pass.
  - Fix: `FocusStore.subscribeWithin(moniker, cb)` (prefix-indexed subtree subscriptions; `set()` notifies only the ancestor paths of prev/next — O(path depth), preserving the per-moniker scaling the store was built for) + `useOptionalIsFocusWithin(fq)` hook (boolean snapshot — re-renders only on containment flips). `useFocusedWebviewCommandHandlers` now gates on focus-within-subtree, matching the keymap's marker-in-chain granularity. 5 new unit tests in `entity-focus-context.test.tsx` (containment, separator-boundary no-false-prefix, wake selectivity, no-provider, empty-key degenerate).
  - Pressable verified: a `<Pressable>` is a spatial leaf (a registered `<FocusScope>` cannot contain another), so subtree containment degenerates to direct focus — same gate serves both surfaces; documented in pressable.tsx.
- WARNING resolved by DELETION: `extractScopeBindings` and `extractScopedRegistryBindings` removed from keybindings.ts (confirmed zero production importers; `use-hotkeys` exists only as a test). Their tests rewritten onto the live `extractChainBindings` path (`extractChainBindings([], mode, scope)` for the component-def walk; a `monikerChain()` helper for the registry-only chains) in keybindings.test.ts, use-hotkeys.test.tsx, badge-list-display.test.tsx.
- NIT (a): all stale doc-comment references to the deleted wrappers across `apps/kanban-app/ui/src` (keybindings.ts:60/97, app-shell.tsx x3, inspectable.tsx, perspective-tab-bar.tsx, badge-list-display.tsx, mock-command-list.ts, and test-file narrative comments) now name `extractChainBindings`. Zero occurrences of the deleted names remain.
- NIT (b): ARCHITECTURE.md gained a "Keybindings, Marker Scopes, and the Webview Command Bus" subsection under Command Invocation Surfaces (chain walk, bus, marker monikers, focus-within gating — concrete file/component names, no internal jargon).
- Verification: browser vitest scoped batch (field.enter-edit, field.read-only, pressable, keybindings, entity-focus-context, use-hotkeys, badge-list-display, badge-list-nav) = 8 files / 173 tests passed; node project (webview-command-bus.guard, ui-surface/grid/nav plugin-commands mirrors) = 4 files / 22 tests passed; pressable end-to-end spatial tests (nav-bar.inspect-enter, left-nav.view-enter, column-view.add-task-enter) = 3 files / 4 tests passed; mock-extended files (grid-view x3, inspectors-container, app-dismiss.topmost-layer) passed — grid-empty-state's single failure is the documented pre-existing one (context-menu assertion, file diff vs HEAD is one mock line). `tsc --noEmit` exit 0. No Rust touched.

## Review Findings (2026-06-11 09:46)

Iteration-2 verification of the blocker fix: all prior items confirmed genuinely resolved. Reviewer independently re-ran the scoped browser batch (8 files / 173 passed), the node mirror+guard batch (4 files / 22 passed), the pressable e2e spatial tests (3 files / 4 passed), and `tsc --noEmit` (exit 0). Red/green probe: reverting `useFocusedWebviewCommandHandlers` to a direct-focus gate makes `enter_on_pill_inside_tags_field_opens_the_field_editor` fail with the exact dead-key assertion; restoring the focus-within gate returns the file to 6/6 green. Separator-boundary containment, subscription cleanup, O(path-depth) wake selectivity, sibling-field disjointness (FQM tree structure; no production nested `<Field>`), and ownership-guarded bus handoff all verified in code and tests. Deletion safety: zero remaining source/test references to `extractScopeBindings` / `extractScopedRegistryBindings`.

Remaining: three doc-comment nits — prose that still describes the PRE-fix direct-focus granularity the blocker fix replaced.

### Nits
- [x] `builtin/plugins/ui-commands/index.ts:175` — `UI_SURFACE_COMMANDS` doc says the owning component registers its bus handler "WHILE ITS ZONE / LEAF IS THE SPATIAL FOCUS"; since the blocker fix, fields register while spatial focus is anywhere WITHIN the zone's subtree (pressables degenerate to direct focus). Align with the wording in `use-focused-webview-command-handlers.ts`.
- [x] `apps/kanban-app/ui/src/test/mock-command-list.ts:160` — same stale phrase: "field.tsx and pressable.tsx only register webview-bus handlers for the ids while their zone/leaf is the spatial focus" — should say while spatial focus is within the surface's subtree.
- [x] `apps/kanban-app/ui/src/components/fields/field.tsx:485` — file-header sentence "claims Enter (cua) / `i` + Enter (vim) for them whenever a field zone is the spatial focus" contradicts the marker-in-chain gate stated two sentences later (the keys also bind while a pill inside the field is focused — that is the whole point of the blocker fix). Match the accurate inline comment at the registration site (field.tsx:556-565).

## Doc-Nit Resolution (2026-06-11, iteration 3)

Comment-only edits, no executable code touched:
- `builtin/plugins/ui-commands/index.ts` — `UI_SURFACE_COMMANDS` doc now reads "WHILE SPATIAL FOCUS IS WITHIN ITS SUBTREE … matching the keymap's marker-in-chain gate (a pressable is a spatial leaf, so containment degenerates to direct focus)", pointing at `use-focused-webview-command-handlers.ts`; the field-edit table comment's "only while a field zone is focused" likewise now says "while the `ui:field` marker is in the focused chain (the field zone itself or a pill inside it)".
- `apps/kanban-app/ui/src/test/mock-command-list.ts` — mirror doc now says handlers register "while spatial focus is within their instance's subtree … matching the keymap's marker-in-chain gate; a pressable is a spatial leaf, so containment degenerates to direct focus".
- `apps/kanban-app/ui/src/components/fields/field.tsx` — header now says the chain walk claims the keys "whenever the `ui:field` marker appears anywhere in the focused scope chain (the field zone itself or a pill inside it)", consistent with the registration-site comment.

Verification: `npx tsc --noEmit` exit 0 (kanban-app UI; builtin/plugins has no standalone tsconfig/typecheck project); `ui-surface-plugin-commands-mirror` node test (parses the edited plugin source from disk) 6/6 passed.