---
assignees:
- claude-code
position_column: todo
position_ordinal: aa80
title: Persist active perspective per view — restore on app restart, default to first
---
## What

Currently `active_perspective_id` is a single `String` on `WindowState` (`swissarmyhammer-commands/src/ui_state.rs:47`). When the user switches from Board view to Grid view, the perspective selection is lost — the same ID is used for both views. The user wants each view to remember its own active perspective, persisted across app restarts, defaulting to the first available perspective when none is stored.

### Approach

Change `active_perspective_id: String` to `active_perspective_per_view: HashMap<String, String>` on `WindowState`, keyed by view ID. The current single field is already persisted to `ui-state.yaml` — the HashMap will persist the same way.

### Files to modify

**Rust (backend):**
- `swissarmyhammer-commands/src/ui_state.rs`
  - Change `active_perspective_id: String` to `active_perspective_per_view: HashMap<String, String>` on `WindowState` (line 47)
  - Update `set_active_perspective()` (line 375) to take `view_id: &str` and write to the HashMap
  - Update `to_json()` (line 924) to include the new field
  - Update default in `WindowState::default()` (line 72)
  - Add `#[serde(default)]` for backward compat with existing YAML files
- `swissarmyhammer-kanban/src/commands/ui_commands.rs`
  - Update `SetActivePerspectiveCmd` (line 177) to read `view_id` from args and pass to `set_active_perspective`
- `swissarmyhammer-commands/builtin/commands/ui.yaml`
  - Add `view_id` param to `ui.perspective.set` command def

**TypeScript (frontend):**
- `kanban-app/ui/src/lib/ui-state-context.tsx`
  - Update `WindowStateSnapshot` interface to have `active_perspective_per_view: Record<string, string>` instead of `active_perspective_id: string`
- `kanban-app/ui/src/lib/perspective-context.tsx`
  - Read `active_perspective_per_view?.[viewKind]` instead of `active_perspective_id`
  - Pass `view_id: viewKind` in the `ui.perspective.set` dispatch args
  - Fallback to first perspective already exists (line 148) — just wire the new key

## Acceptance Criteria
- [ ] Switching views preserves each view's active perspective independently
- [ ] Closing and reopening the app restores the per-view perspective selection
- [ ] When no perspective is stored for a view, defaults to first available
- [ ] Backward compatible — existing `ui-state.yaml` with old `active_perspective_id` doesn't crash

## Tests
- [ ] Rust: `set_active_perspective` with view_id stores per-view; reading back returns correct ID
- [ ] Rust: different view_ids store independently
- [ ] Rust: missing view_id returns empty string (default behavior)
- [ ] TS: `perspective-context.test.tsx` updated for per-view mock data
- [ ] `cargo nextest run -p swissarmyhammer-commands` — all pass
- [ ] `pnpm test` from `kanban-app/ui/` — all pass