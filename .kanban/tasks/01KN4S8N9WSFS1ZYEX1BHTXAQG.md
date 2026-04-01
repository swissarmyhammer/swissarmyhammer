---
assignees:
- claude-code
depends_on:
- 01KN4S7XVZJ7ZZ6MSB7WAG2F99
position_column: todo
position_ordinal: '8880'
title: Implement TrackedStore for Perspective + undoable commands + events
---
## What

Implement `TrackedStore<Item = Perspective, ItemId = PerspectiveId>` for the perspective system. Register perspective store in `StoreContext`. Mark perspective commands undoable. Change events flow through the generic `flush_all()` path.

**Files to modify:**
- `swissarmyhammer-perspectives/Cargo.toml` — add dep on `swissarmyhammer-store`
- `swissarmyhammer-perspectives/src/lib.rs` — add `PerspectiveId(Ulid)` newtype, export it
- `swissarmyhammer-perspectives/src/context.rs` — implement `TrackedStore`:
  - `type Item = Perspective`
  - `type ItemId = PerspectiveId`
  - `root()` → perspectives directory
  - `item_id(perspective)` → `PerspectiveId` from perspective
  - `serialize(perspective)` → YAML text
  - `deserialize(text)` → parse YAML into Perspective
- `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — use `StoreHandle::write()` / `StoreHandle::delete()` instead of `PerspectiveContext` direct writes. Push `UndoEntryId` to `StoreContext`.
- `swissarmyhammer-commands/builtin/commands/perspective.yaml` — add `undoable: true` to: `perspective.save`, `perspective.delete`, `perspective.filter`, `perspective.clearFilter`, `perspective.group`, `perspective.clearGroup`
- `kanban-app/src/state.rs` — register perspective `StoreHandle` in `StoreContext` on board open

**Files to delete:**
- `swissarmyhammer-perspectives/src/changelog.rs` — replaced by store crate

**Approach:**

### PerspectiveId newtype
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PerspectiveId(ulid::Ulid);
// Display, FromStr — delegate to Ulid
```

### TrackedStore impl
- `serialize`: `serde_yaml_ng::to_string(&perspective)` — matches existing `.yaml` format
- `deserialize`: `serde_yaml_ng::from_str(text)` — matches existing parse

### Command migration
Perspective commands currently use `PerspectiveContext::write()` directly. Migrate to:
```rust
let entry_id = store_handle.write(&perspective).await?;
store_context.push(entry_id, format!("save perspective {}", perspective.name)).await;
```

Delete:
```rust
let entry_id = store_handle.delete(&perspective_id).await?;
store_context.push(entry_id, format!("delete perspective {}", name)).await;
```

### Events
No special perspective event emission needed — `store_context.flush_all()` in `dispatch_command_internal` already handles it. The perspective store's `flush_changes()` produces events that flow through the same generic path as entity events.

### Manual test checklist
1. Create perspective → appears on disk, changelog entry logged
2. Cmd+Z → perspective deleted
3. Cmd+Shift+Z → perspective restored
4. Edit filter → Cmd+Z → filter reverted
5. Delete perspective → Cmd+Z → restored
6. Interleaved: create task, create perspective, Cmd+Z undoes perspective, Cmd+Z undoes task

## Acceptance Criteria
- [ ] `PerspectiveId(Ulid)` newtype exported from perspectives crate
- [ ] `TrackedStore<Item=Perspective, ItemId=PerspectiveId>` implemented
- [ ] Perspective serialize/deserialize matches existing YAML format
- [ ] Perspective `changelog.rs` deleted, replaced by store crate
- [ ] Perspective commands use `StoreHandle` for writes/deletes
- [ ] Perspective commands push `UndoEntryId` to `StoreContext`
- [ ] Mutating perspective commands have `undoable: true` in YAML
- [ ] Perspective store registered in `StoreContext`
- [ ] Cmd+Z/Cmd+Shift+Z works for perspective operations
- [ ] Interleaved entity + perspective undo/redo works
- [ ] Change events emitted via generic `flush_all()` path

## Tests
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-perspectives)'` — TrackedStore impl, YAML round-trip
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` — perspective undo/redo integration, interleaved with entity undo
- [ ] `cargo nextest run --workspace` — no regressions
- [ ] Manual: create perspective → undo → redo
- [ ] Manual: interleaved entity + perspective undo sequence