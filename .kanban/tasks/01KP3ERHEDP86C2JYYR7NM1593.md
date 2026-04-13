---
assignees:
- wballard
position_column: todo
position_ordinal: c680
title: 'Perspective switch: backend filters + pushes results to UIState'
---
## What

Today, clicking a perspective tab is a two-step dance that does the wrong thing architecturally:

1. Frontend dispatches `ui.perspective.set` — backend just mutates `UIState.active_perspective_id` (pure UI, no filtering).
2. `ui-state-changed` event fires, frontend re-derives `activePerspective`.
3. `PerspectiveContainer` sees `activeFilter` changed and calls `refreshEntities(boardPath, activeFilter)` in a `useEffect`, which hits `list_entities` on the backend to actually do the filtering.

This means the filter work is happening in a follow-up roundtrip driven by the frontend, not in response to the click. It also means clicking a perspective doesn't trigger the indeterminate progress bar, because the dispatched command (`ui.perspective.set`) has no backend work to track.

**Desired flow:**

1. Frontend dispatches a single backend command — `perspective.switch` — with the perspective id.
2. Backend handler loads the perspective, evaluates its filter against the board's tasks, and in **one** atomic UIState update sets **both** the new `active_perspective_id` and the filtered task id list for this window.
3. `ui-state-changed` event fires carrying the new state. Frontend renders the filtered list directly out of UIState — no separate `refreshEntities` call, no frontend-side filter fetch.

Because the dispatch is the real filter work, the `inflightCount` counter in `CommandBusyProvider` increments for the duration of the filter evaluation, so the indeterminate bar in `NavBar` appears naturally.

### Concrete changes

1. **New command YAML** — `swissarmyhammer-commands/builtin/commands/perspective.yaml`: add
   ```yaml
   - id: perspective.switch
     name: Switch Perspective
     visible: false
     params:
       - name: perspective_id
         from: args
   ```

2. **New Rust handler** — `swissarmyhammer-kanban/src/commands/perspective_commands.rs`: add `SwitchPerspectiveCmd`. Register it in `swissarmyhammer-kanban/src/commands/mod.rs` alongside the other perspective commands. The handler must:
   - Read `perspective_id` from args.
   - Look up the perspective definition (filter, view kind).
   - Load the window's board tasks and evaluate the perspective's filter DSL against them to produce the filtered task id list. Reuse the same filter evaluation code path `list_entities` uses (see `kanban-app/src/commands.rs:417-440`) — do **not** duplicate the DSL evaluator.
   - Update UIState with a single change covering both `active_perspective_id` and `filtered_task_ids` for the window (see UIState changes below).
   - Return the `UIStateChange`.

3. **UIState per-window field** — add `filtered_task_ids: Vec<String>` to the per-window state struct in UIState (Rust) and mirror in `WindowStateSnapshot` TypeScript type in `kanban-app/ui/src/lib/ui-state-context.tsx`. Bump serialization/tests so existing snapshots deserialize (default empty vec).

4. **UIState setter** — extend the UIState API with a single method that sets both `active_perspective_id` and `filtered_task_ids` atomically so one `UIStateChange` is produced and one `ui-state-changed` event fires. Don't emit two events.

5. **Frontend dispatch** — `kanban-app/ui/src/lib/perspective-context.tsx`: change `setActivePerspectiveId` to use the **pre-bound** form of `useDispatchCommand` against the new command:
   ```tsx
   const dispatchSwitch = useDispatchCommand("perspective.switch");
   const setActivePerspectiveId = useCallback(
     (id: string) => {
       dispatchSwitch({ args: { perspective_id: id } }).catch(console.error);
     },
     [dispatchSwitch],
   );
   ```

6. **Remove frontend-side filter fetch** — `kanban-app/ui/src/components/perspective-container.tsx`: delete the `useEffect` that calls `refreshEntities(boardPath, activeFilter)` on perspective/filter change. The backend now owns this. (Note: the filter formula bar — editing a perspective's filter in place — is a separate path via `perspective.filter` that already updates the perspective entity; that flow still needs to eventually re-apply the filter. Out of scope here unless trivial; file a follow-up if touching it would exceed sizing.)

7. **Frontend reads filtered tasks from UIState** — wire `WindowStateSnapshot.filtered_task_ids` through to whatever context currently feeds `board-view` / `grid-view` the task list. Simplest: intersect `entitiesByType.task` with `filtered_task_ids` in a selector near `view-container.tsx:57` (`GroupedBoardView`).

8. **Deprecate `ui.perspective.set`** — the YAML entry in `swissarmyhammer-commands/builtin/commands/ui.yaml` and its handler `SetActivePerspectiveCmd` in `swissarmyhammer-kanban/src/commands/ui_commands.rs` become dead. Remove both (and their tests). If another caller exists, migrate it to `perspective.switch`. Grep before deleting.

### Out of scope

- Filter editor / formula-bar re-application path.
- Sort and group application — those remain client-side (`evaluateSort` in `perspective-container.tsx`), matching current behavior.
- Other UI-only commands (`ui.view.set`, `ui.mode.set`) — leave them alone.

## Acceptance Criteria

- [ ] `perspective.switch` command exists in `perspective.yaml` and is handled by `SwitchPerspectiveCmd`; `ui.perspective.set` is removed (both YAML and handler).
- [ ] Backend atomically updates `active_perspective_id` and `filtered_task_ids` on the window in a single `UIStateChange`; exactly one `ui-state-changed` event fires per click.
- [ ] Clicking a perspective tab in the running app visibly shows the indeterminate progress bar under `NavBar` for the duration of the filter evaluation.
- [ ] The previously-needed `useEffect` in `perspective-container.tsx` that called `refreshEntities(boardPath, activeFilter)` on perspective change is gone; perspective change no longer triggers a second roundtrip from the frontend.
- [ ] Views render the filtered task list derived from `WindowStateSnapshot.filtered_task_ids`; switching perspectives updates what is visible without any frontend-side filter call.
- [ ] `cargo test -p swissarmyhammer-kanban` and `bun run test` (in `kanban-app/ui`) pass. `bun run typecheck` passes.

## Tests

- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — add unit tests for `SwitchPerspectiveCmd` covering: (a) sets `active_perspective_id`, (b) writes filtered task ids matching the perspective's filter, (c) both changes land in one `UIStateChange`, (d) unknown perspective id returns a clean `ExecutionFailed` error.
- [ ] Rust integration test (same crate or `swissarmyhammer-kanban/tests/`) that dispatches `perspective.switch` via the command registry end-to-end and asserts the resulting UIState snapshot.
- [ ] `kanban-app/ui/src/lib/perspective-context.test.tsx` — update the existing "dispatches ui.perspective.set to backend" test to assert dispatch of `perspective.switch` with `{ args: { perspective_id } }` and no `"ui.perspective.set"` IPC. Delete tests that assert the old command name.
- [ ] Add a test in `kanban-app/ui/src/components/perspective-container.test.tsx` (create if absent) that mounts `PerspectiveContainer` and asserts it does NOT call `refreshEntities` on perspective change — the filtered list comes from UIState.
- [ ] Add an integration-style test in `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` that clicks a tab, mocks the `dispatch_command` invoke to pend for ~50ms, and asserts `role="progressbar"` is visible during the pending window. (This replaces the older "doesn't show busy" smoke concern with a real assertion.)
- [ ] Commands to run: `cargo nextest run -p swissarmyhammer-kanban`, `bun run test -- perspective-context perspective-container perspective-tab-bar`, `bun run typecheck`. All green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #bug #unified-commands #ui-state