---
assignees:
- claude-code
depends_on:
- 01KN2Q4ZRV7651J5XCQ4BXETKK
position_column: done
position_ordinal: ffffffffffffffffff9280
title: 'PERSP-2: PerspectiveContext storage layer'
---
## What

Create `swissarmyhammer-kanban/src/perspective/context.rs` with `PerspectiveContext` — file-backed CRUD for `.kanban/perspectives/{id}.yaml`. Model after existing patterns (e.g., how tags/actors work with their own storage dirs).

**PerspectiveContext methods:**
- `open(dir: PathBuf) -> Self` — load all YAML files from dir into memory
- `write(&mut self, perspective: &Perspective)` — write YAML, update in-memory index
- `get_by_id(&self, id: &str) -> Option<&Perspective>`
- `get_by_name(&self, name: &str) -> Option<&Perspective>`
- `all(&self) -> &[Perspective]`
- `delete(&mut self, id: &str) -> Result<Perspective>` — remove file, return deleted for changelog

**KanbanContext changes** (`swissarmyhammer-kanban/src/context.rs`):
- Add `perspectives_dir()` method returning `self.root.join("perspectives")`
- Create `perspectives/` in `create_directories()`
- Check `perspectives/` in `directories_exist()`
- Load `PerspectiveContext` in board open flow

## Acceptance Criteria
- [ ] PerspectiveContext writes perspectives as YAML to `perspectives/{id}.yaml`
- [ ] Read back by ID and by name
- [ ] List returns all perspectives
- [ ] Delete removes file and returns the deleted perspective
- [ ] Survives context reopen (persistence test)
- [ ] KanbanContext creates `perspectives/` directory

## Tests
- [ ] `write_and_read_by_id`
- [ ] `write_and_read_by_name`
- [ ] `list_all_perspectives`
- [ ] `delete_perspective`
- [ ] `delete_nonexistent_errors`
- [ ] `persistence_survives_reopen`
- [ ] `kanban_context_creates_perspectives_dir`
- [ ] Run: `cargo test -p swissarmyhammer-kanban perspective::context`