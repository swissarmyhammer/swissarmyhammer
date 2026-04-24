---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
position_column: todo
position_ordinal: ff8b80
project: spatial-nav
title: Drill-in/drill-out commands + Enter→drillIn, Space→inspect (newtype signatures)
---
## What

Add explicit drill-in and drill-out commands to complement the three-rule beam search. Arrow keys handle ordinary nav; these commands give access to zone-level focus (drill-out) and into-zone descent (drill-in), matching the nested zone model. All signatures use newtypes — `Option<Moniker>` returns, `SpatialKey` inputs.

Key-chord change: **Inspect moves from Enter to Space**, freeing Enter for drill-in / activate.

### Crate placement

Per the commit-`b81336d42` refactor pattern:
- `drill_in` / `drill_out` methods on `SpatialRegistry` in `swissarmyhammer-kanban/src/focus/registry.rs` (or a `focus/drill.rs` if it grows)
- Tauri adapters `spatial_drill_in` / `spatial_drill_out` in `kanban-app/src/commands.rs`
- React commands and keybinding changes in `kanban-app/ui/src/`
- Tests in `swissarmyhammer-kanban/tests/focus_drill.rs`

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
- [ ] Add Rust `drill_in` / `drill_out` with newtyped signatures (`SpatialKey` → `Option<Moniker>`)
- [ ] Add Tauri commands `spatial_drill_in` / `spatial_drill_out`
- [ ] Add React `nav.drillIn` (Enter) and `nav.drillOut` (Escape) commands using branded `SpatialKey` / `Moniker`
- [ ] Integrate `inspector.edit` and field-row edit with `nav.drillIn` (Enter still edits on a field-level leaf with an editor)
- [ ] Rebind `ui.inspect` (and analogues) to Space (CUA)
- [ ] Verify Escape chain still closes modals at layer root
- [ ] Update the keybinding documentation / command descriptions

## Acceptance Criteria
- [ ] All new signatures use newtypes (`SpatialKey` inputs, `Option<Moniker>` outputs) — no bare strings
- [ ] Enter on a Zone → drills into its `last_focused` or first child
- [ ] Enter on a Leaf with edit affordance → inline edit
- [ ] Enter on a Leaf without edit affordance → no-op (does not open inspector)
- [ ] Escape on a Leaf → focuses its `parent_zone`
- [ ] Escape on a Zone → focuses its `parent_zone` if any
- [ ] Escape at layer root falls through to existing dismiss / close-modal
- [ ] Space on any focused entity → opens inspector (`ui.inspect` routed by `Moniker`)
- [ ] `cargo test` and `pnpm vitest run` pass

## Tests
- [ ] Rust: `drill_in` on a Zone with live `last_focused` returns that entry's `Moniker`
- [ ] Rust: `drill_in` on a Zone whose `last_focused` is stale returns the first-child `Moniker`
- [ ] Rust: `drill_in` on a Focusable returns `None`
- [ ] Rust: `drill_out` on a Focusable returns its `parent_zone`'s `Moniker`
- [ ] Rust: `drill_out` on a Zone returns its `parent_zone`'s `Moniker`, or `None` at layer root
- [ ] React: Enter on a focused zone invokes `spatial_drill_in` and `setFocus` with the returned `Moniker`
- [ ] React: Escape on a focused leaf invokes `spatial_drill_out`; on `null`, falls through to dismiss
- [ ] React: Space key triggers `ui.inspect` for the focused `Moniker`, opening the inspector panel
- [ ] React: Enter on a task-card rename leaf enters inline rename (not inspector)
- [ ] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.