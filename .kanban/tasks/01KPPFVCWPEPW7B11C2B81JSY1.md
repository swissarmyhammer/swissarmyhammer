---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8680
title: 'Commands: delete TypeScript EntityCommand / EntityCommandKeys and their consumers'
---
## What

Follow-up from `01KPJSSVCW774TK2E2JSMD3Y1J` (retire Rust EntityCommand). The TypeScript parallel types are still in place even though the backend no longer serves entity-scoped commands via entity schemas.

### Dead types in `kanban-app/ui/src/types/kanban.ts`

- `interface EntityCommand` (lines ~6-18)
- `interface EntityCommandKeys`
- `EntityDef.commands` field (around line 191)

### Consumers to audit and rewire

- `kanban-app/ui/src/lib/entity-commands.ts` — `buildEntityCommandDefs` et al
- `kanban-app/ui/src/lib/entity-commands.test.ts`
- `kanban-app/ui/src/lib/schema-context.tsx` — exposes entity commands to the React tree
- `kanban-app/ui/src/components/*-view.tsx` — any `useEntityCommands` consumers
- Associated tests (grid-view, board-view, column-view, entity-card, data-table, command-palette, etc.)

### Approach

1. Confirm the resolved command list is already delivered through `commands_for_scope` (Rust) and the frontend reads only that list. The entity-schema `commands:` pass has been removed server-side, so anything `buildEntityCommandDefs` produces is dead.
2. Delete `interface EntityCommand`, `interface EntityCommandKeys`, and `EntityDef.commands`.
3. Delete `buildEntityCommandDefs`, `useEntityCommands`, and the `entity-commands.{ts,test.ts}` module if nothing else uses them.
4. Audit `schema-context.tsx` for entity-command plumbing; strip it.
5. Run `npm test` / `vitest` and `tsc --noEmit` in `kanban-app/ui` to prove nothing references the deleted types.

### Acceptance Criteria

- [ ] `grep -rn "EntityCommand\\|EntityCommandKeys" kanban-app/ui/src` returns zero matches.
- [ ] `EntityDef` TS interface has no `commands` field.
- [ ] `entity-commands.ts` is deleted (or heavily slimmed) and its tests updated.
- [ ] Frontend build (`tsc --noEmit` in kanban-app/ui) passes.
- [ ] Frontend test suite passes.

## Workflow

Use `/tdd` to verify behavior is preserved: the context menu / command palette for each entity should still surface the same commands it does today, since those come from `commands_for_scope`, not from the TS entity schema.

#commands #frontend

## Review Findings (2026-04-20 21:20)

### Warnings

- [x] `kanban-app/ui/src/components/{avatar,board-view,column-view,command-palette,data-table,inspector-focus-bridge}.tsx` (and others) — every rewired `FocusScope commands={[]}` passes a fresh `[]` literal on every render. `FocusScope` (focus-scope.tsx:89-103) uses that array as a `useMemo` dep that rebuilds a `Map<string, CommandDef>` and a `useEffect` dep that re-invokes `registerScope`/`unregisterScope`. Before this change, `useEntityCommands` returned a memoized array, so the scope Map was stable across renders. Now every FocusScope in these paths churns its scope registration on every render. Fix: hoist a module-level `const EMPTY_COMMANDS: readonly CommandDef[] = []` (in `command-scope.ts` or `focus-scope.tsx`) and use it from all the rewired call sites, or make `FocusScope.commands` optional and default to that shared constant inside the component.
  - Fixed: added exported `EMPTY_COMMANDS = Object.freeze([])` in `kanban-app/ui/src/lib/command-scope.tsx`; made `commands` optional on both `FocusScope` and `CommandScopeProvider` with `EMPTY_COMMANDS` as the default; stripped `commands={[]}` from every production call site so the scope registration is stable across renders.
- [x] `kanban-app/ui/src/components/focus-scope.tsx:34` — with no production caller now passing a dynamic commands array (every rewired site is `[]`; the only non-`[]` sites like `entity-card` and `mention-view` just forward an `extraCommands` prop), the `commands` prop's design justification is weaker. Consider whether `FocusScope` should drop the `commands` prop entirely and let the handful of callers that need per-scope commands compose a child `CommandScopeProvider`, or at minimum make the prop optional. Not blocking, but the current shape invites more `commands={[]}` noise as new FocusScope sites are added.
  - Fixed: `FocusScope.commands` is now optional (typed `readonly CommandDef[] | undefined`) and defaults to `EMPTY_COMMANDS`. Kept the prop (rather than dropping it) because `entity-card` and `mention-view` legitimately forward `extraCommands` — moving them to a child `CommandScopeProvider` would add a scope level and change `FocusedScopeContext` behavior for those entities.

### Nits

- [x] `kanban-app/ui/src/components/command-palette.tsx:479-484` — the doc comment on `SearchResultItem` was updated from "hooks cannot be called inside `.map()`" to "scope registration cannot be done inside `.map()`". Accurate (FocusScope still uses `useEffect`), but with the per-row hook call gone, `SearchResultItem` is now a thin wrapper around `SearchResultRow`; consider collapsing them into one component to reduce indirection.
  - Fixed: collapsed `SearchResultItem` + `SearchResultRow` into a single `SearchResultItem` component in `command-palette.tsx`. `useDispatchCommand` now lives at the top of `SearchResultItem` (still a top-level component, not called inside `.map()`), so scope registration via `FocusScope`'s `useEffect` is still outside the loop. Doc comment rewritten to explain the top-level placement.
