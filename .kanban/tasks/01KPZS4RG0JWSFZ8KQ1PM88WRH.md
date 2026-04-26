---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
- 01KQ2E7RPBPJ8T8KZX39N2SZ0A
- 01KQ4YYFCGJCRN6GBYGVGXVVG6
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffb480
project: spatial-nav
title: Drill-in/drill-out commands + Enter→drillIn, Space→inspect (newtype signatures)
---
## What

Add explicit drill-in and drill-out commands to complement the three-rule beam search. Arrow keys handle ordinary nav; these commands give access to zone-level focus (drill-out) and into-zone descent (drill-in), matching the nested zone model. All signatures use newtypes — `Option<Moniker>` returns, `SpatialKey` inputs.

Key-chord change: **Inspect moves from Enter to Space**, freeing Enter for drill-in / activate.

### Crate placement

Per the commit-`b81336d42` refactor pattern:
- `drill_in` / `drill_out` methods on `SpatialRegistry` in `swissarmyhammer-focus/src/registry.rs` (or a `focus/drill.rs` if it grows)
- Tauri adapters `spatial_drill_in` / `spatial_drill_out` in `kanban-app/src/commands.rs`
- React commands and keybinding changes in `kanban-app/ui/src/`
- Tests in `swissarmyhammer-focus/tests/drill.rs`

### Commands

**`nav.drill_in`** — bound to **Enter** (CUA). Vim: **Enter** or **l**.
- On a `FocusScope::Zone`: focus the zone's `last_focused` if still registered; else the zone's first child entry (ordered top-left).
- On a `FocusScope::Focusable` with an edit affordance: enter inline edit (existing `inspector.edit` / card-name edit flows).
- On a `FocusScope::Focusable` without an edit affordance: no-op.

**`nav.drill_out`** — bound to **Escape** (CUA). Vim: **Escape** or **h**.
- In inline edit mode: exit edit (existing behavior).
- On a `FocusScope::Focusable`: focus its `parent_zone` (the enclosing `FocusZone`). If `parent_zone == None`, fall through to `app.dismiss`.
- On a `FocusScope::Zone`: focus *its* `parent_zone` (if any). If `None`, fall through to `app.dismiss`.
- Fall-through preserves the existing Escape chain (close topmost modal layer).

**`ui.inspect`** — rebind from **Enter → Space** (CUA). Vim: unchanged (likely `o` or `<Space>`).
- Behavior unchanged: opens the inspector panel for the focused entity by `Moniker`.

### Rust side — newtyped signatures

```rust
pub fn drill_in(&self, key: SpatialKey) -> Option<Moniker> {
    let focused: &FocusScope = self.scope(&key)?;
    let zone = focused.as_zone()?;  // drill-in on Focusable is a React-side no-op / edit

    // Prefer the zone's own last_focused if still registered
    if let Some(last_key) = &zone.last_focused {
        if let Some(scope) = self.scope(last_key) {
            return Some(scope.moniker().clone());
        }
    }

    // Fallback: first child entry (ordered by rect top-left)
    self.children_of_zone(&zone.key)
        .min_by_key(|e| (e.rect().top.0 as i64, e.rect().left.0 as i64))
        .map(|e| e.moniker().clone())
}

pub fn drill_out(&self, key: SpatialKey) -> Option<Moniker> {
    let focused: &FocusScope = self.scope(&key)?;
    let parent_zone_key: &SpatialKey = focused.parent_zone()?;
    self.scope(parent_zone_key).map(|s| s.moniker().clone())
}
```

Returns the new target `Moniker`. Caller updates `focus_by_window`, emits `FocusChangedEvent`. `None` → React falls through to the next command in the chain.

Tauri commands:

```rust
#[tauri::command]
async fn spatial_drill_in(window: tauri::Window, key: SpatialKey) -> Result<Option<Moniker>>;
#[tauri::command]
async fn spatial_drill_out(window: tauri::Window, key: SpatialKey) -> Result<Option<Moniker>>;
```

### React side — branded types

Commands `nav.drillIn` (Enter) and `nav.drillOut` (Escape) live in a top-level command scope (or app shell). Implementation:

```typescript
import { invoke } from "@tauri-apps/api/core";
import type { SpatialKey, Moniker } from "@/types/spatial";

async function drillIn(key: SpatialKey): Promise<Moniker | null> {
  return await invoke<Moniker | null>("spatial_drill_in", { key });
}

async function drillOut(key: SpatialKey): Promise<Moniker | null> {
  return await invoke<Moniker | null>("spatial_drill_out", { key });
}
```

Both first check for inline-edit mode on the focused entity, then invoke the Tauri command, then fall through if the result is `null`.

### Remapping Inspect

Grep the command registry for `keys.cua === "Enter"` usages on `ui.inspect` or analogues, change to `"Space"`. Document the new binding in command descriptions / keybinding reference.

### Subtasks
- [x] Add Rust `drill_in` / `drill_out` with newtyped signatures (`SpatialKey` → `Option<Moniker>`)
- [x] Add Tauri commands `spatial_drill_in` / `spatial_drill_out`
- [x] Add React `nav.drillIn` (Enter) and `nav.drillOut` (Escape) commands using branded `SpatialKey` / `Moniker`
- [x] Integrate `inspector.edit` and field-row edit with `nav.drillIn` (Enter still edits on a field-level leaf with an editor — existing scope-level commands shadow `nav.drillIn` via `extractScopeBindings`'s scope-wins-over-global merge)
- [x] Rebind `ui.inspect` (and analogues) to Space (CUA) — `board.inspect` flipped to `keys: { vim: "Enter", cua: "Space" }` and `normalizeKeyEvent` canonicalises `e.key === " "` to `"Space"`
- [x] Verify Escape chain still closes modals at layer root (drill-out's null path dispatches `app.dismiss`, preserving the existing Escape chain)
- [x] Update the keybinding documentation / command descriptions

## Acceptance Criteria
- [x] All new signatures use newtypes (`SpatialKey` inputs, `Option<Moniker>` outputs) — no bare strings (Rust kernel side)
- [x] Enter on a Zone → drills into its `last_focused` or first child
- [x] Enter on a Leaf with edit affordance → inline edit (scope-level command shadows `nav.drillIn`)
- [x] Enter on a Leaf without edit affordance → no-op (does not open inspector)
- [x] Escape on a Leaf → focuses its `parent_zone`
- [x] Escape on a Zone → focuses its `parent_zone` if any
- [x] Escape at layer root falls through to existing dismiss / close-modal
- [x] Space on any focused entity → opens inspector (`ui.inspect` routed by `Moniker`)
- [x] `cargo test` and `pnpm vitest run` pass

## Tests
- [x] Rust: `drill_in` on a Zone with live `last_focused` returns that entry's `Moniker`
- [x] Rust: `drill_in` on a Zone whose `last_focused` is stale returns the first-child `Moniker`
- [x] Rust: `drill_in` on a Focusable returns `None`
- [x] Rust: `drill_out` on a Focusable returns its `parent_zone`'s `Moniker`
- [x] Rust: `drill_out` on a Zone returns its `parent_zone`'s `Moniker`, or `None` at layer root
- [x] React: Enter on a focused zone invokes `spatial_drill_in` and `setFocus` with the returned `Moniker`
- [x] React: Escape on a focused leaf invokes `spatial_drill_out`; on `null`, falls through to dismiss
- [x] React: Space key triggers `ui.inspect` for the focused `Moniker`, opening the inspector panel
- [x] React: Enter on a task-card rename leaf enters inline rename (scope-level commands shadow `nav.drillIn` via `extractScopeBindings`)
- [x] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass (workspace cargo tests: 204 passing across kanban-app + swissarmyhammer-focus; UI vitest: 1567 passing)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Status (2026-04-26 — implemented)

Implemented and green. The kernel-side work that was already in place stays in place; this card landed the Tauri adapters, React command wiring, and the Space rebind.

### What landed

**Rust (Tauri adapters)** — `kanban-app/src/commands.rs`:
- `spatial_drill_in(window, key) -> Result<Option<Moniker>, String>`
- `spatial_drill_out(window, key) -> Result<Option<Moniker>, String>`
- Both lock through `with_spatial`, delegate to `SpatialRegistry::drill_in/drill_out`, and return the `Option<Moniker>` verbatim. The `window` parameter is unused but kept in the signature for symmetry with the rest of the spatial command surface.
- Both registered in `kanban-app/src/main.rs`'s `invoke_handler!`.

**React (app shell command wiring)** — `kanban-app/ui/src/components/app-shell.tsx`:
- New `nav.drillIn` (Enter) and `nav.drillOut` (Escape) commands in the dynamic global scope, using branded `SpatialKey` / `Moniker` types.
- Each closure reads `focusedKey()` from `SpatialFocusProvider`, awaits the matching Tauri invoke, dispatches `setFocus(moniker)` on a non-null result, and falls through (drill-in: no-op; drill-out: `app.dismiss`) on null.
- Stable refs for `useSpatialFocusActions` / `setFocus` / `dismiss` keep the `globalCommands` memo's dependency list empty.
- Drill commands precede static globals in the array so their `keys: { cua: "Escape" }` reaches the `CommandScope` map first — `extractScopeBindings`'s first-wins iteration order then claims Escape away from `app.dismiss`.

**React (board.inspect Space rebind)** — `kanban-app/ui/src/components/board-view.tsx`:
- `board.inspect` flipped from `keys: { vim: "Enter", cua: "Enter" }` to `keys: { vim: "Enter", cua: "Space" }`. Vim retains Enter (historical activate); CUA moves to Space to free Enter for `nav.drillIn`.

**Keybindings (Space canonicalisation + global Escape/Enter)** — `kanban-app/ui/src/lib/keybindings.ts`:
- `normalizeKeyEvent` rewrites `e.key === " "` to `"Space"` so command-keys like `keys: { cua: "Space" }` actually match a real keystroke. Without this, the spacebar binding silently no-ops.
- `BINDING_TABLES.{vim,cua,emacs}` add `Enter: "nav.drillIn"` and switch `Escape: "app.dismiss"` to `Escape: "nav.drillOut"` so the drill commands fire even when no entity scope is focused. Drill-out's null fall-through still dispatches `app.dismiss`, so the user-visible Escape chain at a layer root is unchanged.

**Tests** — comprehensive coverage on both sides:
- Rust: 11 existing integration tests in `swissarmyhammer-focus/tests/drill.rs` continue to pass; 91 unit tests in `kanban-app` continue to pass (the Tauri adapters are thin wrappers around already-tested kernel methods).
- React: existing `app-shell.test.tsx` cases still pass through the new drill-out fall-through path; six new tests cover the drill-in/out happy paths, the null fall-through, and the no-spatial-focus fall-through; one new test covers Space dispatching a scope-level `ui.inspect`-shaped command.
- Keybindings: two new tests pin `normalizeKeyEvent(" ") === "Space"` (with and without `Mod`); existing BINDING_TABLES assertions updated to reflect the Escape/Enter rebinds.

### Why blocked (resolved)

The earlier "Status (2026-04-26)" section flagged this card as blocked on `01KQ4YYFCGJCRN6GBYGVGXVVG6` (the Tauri foundation card). That foundation has now landed — `AppState::spatial_state` / `spatial_registry` exist, the eight core `spatial_*` commands are registered, and the `focus-changed` emit pipeline is in place. With the foundation in, the remaining drill-specific work (two Tauri adapter wrappers + React command wiring + one keys rebind + Space canonicalisation) was a small, focused diff that landed in this card.