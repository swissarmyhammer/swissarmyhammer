---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffb980
title: 'Rust-side UndoStack: YAML-persisted on disk'
---
## What

Add an `UndoStack` struct to `swissarmyhammer-entity/src/undo_stack.rs` that tracks an ordered list of transaction/entry ULIDs with a pointer. The stack is persisted to a YAML file on disk inside the `.kanban/` directory so it is human-readable, debuggable, and survives app restarts.

**On-disk format:** `.kanban/undo_stack.yaml`
```yaml
# Undo/redo history — do not commit to version control
max_size: 100
pointer: 3
entries:
  - id: 01ABCDEF...
    label: "Update task title"
    timestamp: "2026-03-28T12:00:00Z"
  - id: 01ABCDEG...
    label: "Move task to Done"
    timestamp: "2026-03-28T12:01:00Z"
  - id: 01ABCDEH...
    label: "Delete task"
    timestamp: "2026-03-28T12:02:00Z"
```

**Key design:**
- `entries: Vec<UndoEntry>` where `UndoEntry { id: String, label: String, timestamp: String }`
- `pointer: usize` — one past last executed (entries[0..pointer) are undoable, entries[pointer..] are redoable)
- `max_size: usize` (default 100)
- `push(entry)` — append, discard redo side, trim if over max
- `undo_target() -> Option<&UndoEntry>` — entry at pointer-1
- `redo_target() -> Option<&UndoEntry>` — entry at pointer
- `can_undo() -> bool`, `can_redo() -> bool`
- `load(path) -> Self` — read from YAML file (or create empty if missing)
- `save(path)` — write YAML to disk
- Derive `Serialize, Deserialize` for YAML round-trip via `serde_yaml_ng`

**Gitignore:** There is NO existing `.gitignore` in the board init flow. Two options:
- Option A: Create `.kanban/.gitignore` during board init with `undo_stack.yaml` entry (modify `swissarmyhammer-kanban/src/board/init.rs`)
- Option B: Add `undo_stack.yaml` to the repo-level `.gitignore` pattern and document it
- Prefer Option A — the board init already creates directories, adding a .gitignore there is natural

**Files to modify:**
- `swissarmyhammer-entity/src/undo_stack.rs` (new) — the stack struct, serialization, unit tests
- `swissarmyhammer-entity/src/lib.rs` — add module + re-export
- `swissarmyhammer-entity/src/context.rs` — add `undo_stack: UndoStack` field, load on EntityContext creation via `self.root().join("undo_stack.yaml")`
- `swissarmyhammer-kanban/src/board/init.rs` — create `.kanban/.gitignore` with `undo_stack.yaml` during board init

## Acceptance Criteria
- [ ] UndoStack struct with push/undo_target/redo_target/can_undo/can_redo
- [ ] Push discards redo side, trims oldest when over capacity
- [ ] Serializes to/from YAML via serde_yaml_ng
- [ ] `load()` reads existing file or returns empty stack
- [ ] `save()` writes human-readable YAML with entries, pointer, max_size
- [ ] Board init creates `.kanban/.gitignore` with `undo_stack.yaml`
- [ ] EntityContext loads the stack on construction via `root().join("undo_stack.yaml")`

## Tests
- [ ] `swissarmyhammer-entity/src/undo_stack.rs` — unit tests: push, undo_target, redo_target, capacity trimming, redo discard, YAML round-trip (serialize → deserialize)
- [ ] `cargo nextest run -p swissarmyhammer-entity` passes