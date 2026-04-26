---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffcb80
title: Perspectives don't load — add integration test with sample data, debug end-to-end
---
## What

Perspectives exist on disk in `.kanban/perspectives/` but the UI shows nothing. The tab bar is empty, "+" does nothing visible, there are no command palette entries, and no tooltip on "+".

The FIRST thing to do is prove whether `perspective.list` returns data in an integration test, then trace the full pipeline.

### Integration test approach

**1. Add perspective sample data to `createTestBoard`** in `kanban-app/ui/src/test/integration-commands.ts`:
- After creating tasks, call `kanban(dir, 'perspective add --name "Sprint" --view board')` and `kanban(dir, 'perspective add --name "Grid View" --view grid"')`
- Return perspective IDs and stripped perspective data alongside tasks/columns

**2. Add browser integration test** in `kanban-app/ui/src/components/board-integration.browser.test.tsx`:
- Verify `perspective.list` returns the created perspectives
- Verify `PerspectiveTabBar` renders tabs for perspective matching the current view kind
- Verify clicking "+" creates a new perspective (file appears on disk + tab appears)

**3. Debug the mock setup** — the current `mockInvoke` in the browser test doesn't handle `dispatch_command` for `perspective.list`. It returns `null` for unknown commands (line 64). The `PerspectiveProvider` calls `backendDispatch({ cmd: "perspective.list" })` which invokes `dispatch_command`. The mock needs to return `{ perspectives: [...], count: N }`.

**4. Also add a Rust integration test** for the command + store + context pipeline:
- In `swissarmyhammer-kanban/tests/perspective_integration.rs`, add a test with StoreHandle wired in that proves: `add perspective` → `list perspectives` → results include the added one → `flush_changes()` has pending events

**5. Quick fixes:**
- `perspective-tab-bar.tsx:154` — add `title="New perspective"` to "+" button  
- `perspective.yaml` — add `palette: true` to `perspective.save`, `perspective.delete`, `perspective.list`

### Files to modify
- `kanban-app/ui/src/test/integration-commands.ts` — add `createPerspective` command and add perspectives to `createTestBoard`
- `kanban-app/ui/src/components/board-integration.browser.test.tsx` — add perspective loading test
- `kanban-app/ui/src/components/perspective-tab-bar.tsx:154` — add tooltip
- `swissarmyhammer-commands/builtin/commands/perspective.yaml` — add `palette: true`
- `swissarmyhammer-kanban/tests/perspective_integration.rs` — add store-wired test

## Acceptance Criteria
- [ ] Browser integration test creates perspectives on disk and verifies they appear in `PerspectiveTabBar`
- [ ] Rust integration test proves perspective.list returns data through the command+store pipeline
- [ ] "+" button shows tooltip "New perspective"
- [ ] `perspective.save` and `perspective.delete` appear in command palette
- [ ] Verified with real board at `~/Desktop`

## Tests
- [ ] New browser test: "renders perspective tabs from real .kanban data"
- [ ] New Rust test: add → list → verify events via StoreHandle
- [ ] `pnpm test` from `kanban-app/ui/` — all pass
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — perspective tests pass