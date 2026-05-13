---
assignees:
- claude-code
position_column: review
position_ordinal: '80'
title: Group dropdown is empty — runtime FieldDefs are missing groupable flag
---
## What

**ITERATION 3 (2026-05-13)**. Iter-1 fixed source location (entity schema vs perspective.fields[]). Iter-2 fixed entity-type derivation under legacy by-kind ambiguity with an active-view tiebreaker. Both passed their tests. The user **still sees an empty Group By popover in the live UI**.

This means the iter-2 test still does not reproduce the user's actual data path. My hypotheses have been wrong twice. **Stop guessing.** This iteration is about observing real production data.

## Approach: observe before fixing

The user's setup is the source of truth. Get telemetry from THEIR run, then fix the exact stage that's broken.

### Step 1 — Add prominent dev-server logging

Add temporary `tracing::info!` (level so it shows by default) at these exact points, with stable prefixes for grep:

- `swissarmyhammer-kanban/src/dynamic_sources.rs::gather_perspectives` — log: `[group-debug] gather_perspectives: active_window_label={?}, active_view_id={?}, views_count={?}`
- `swissarmyhammer-kanban/src/dynamic_sources.rs::denormalize_perspective_fields` — log: `[group-debug] denormalize: persp.id={?}, persp.view={?}, persp.view_id={?}, active_view_id={?}, entity_type={?}, fields_returned={count}`
- `swissarmyhammer-kanban/src/dynamic_sources.rs::entity_type_for_perspective` — log: `[group-debug] entity_type_for_perspective: tier=strict|tiebreaker|by-kind, result={?}`
- `swissarmyhammer-perspectives/src/options_resolvers.rs::PerspectiveFieldsResolver::resolve` — log: `[group-debug] resolver: persp_id_from_scope={?}, options_count={?}`
- `kanban-app/src/commands.rs::list_commands_for_scope` — log: `[group-debug] list_commands_for_scope: scope_chain={?}, returned_count={?}; for each command with options_from=perspective.fields, log: cmd_id={?}, options.len={?}`

Add a console.log in `<CommandPopover>` (`kanban-app/ui/src/components/command-popover.tsx`):
- On render, log: `console.log("[group-debug] CommandPopover render", { commandId: command.id, params: command.params })`

Add a console.log in `useScopedTabCommands` (`kanban-app/ui/src/components/perspective-tab-bar.tsx`):
- On result: `console.log("[group-debug] useScopedTabCommands result", { scopeChain, tabCommands })`

### Step 2 — Push the instrumented build

Commit the logging WITHOUT a fix. The user will rebuild + open the Group popover. Ask them to:
- Open the dev server console (or the Tauri app's webview dev tools).
- Click the Group icon on the perspective tab bar.
- Paste back the `[group-debug]` lines from both the backend (dev server stdout/stderr) AND the frontend (webview console).

### Step 3 — Diagnose from real data

The logs will show:
- Which stage drops the options (backend resolver returns 0? Frontend never receives them? Frontend receives them but doesn't render?)
- What the user's actual `persp.view_id`, `persp.view`, `active_view_id`, and `entity_type` values are.
- The exact scope chain the frontend sends.

That data localizes the bug. Fix THAT stage.

### Step 4 — Write a test that reproduces the EXACT logged values

Before fixing: extend the iter-2 test fixture to use whatever values the user's logs reveal. Confirm the test fails with the current code AND the user's exact values. Confirm the test passes after the fix.

### Step 5 — Remove the logging

Once the fix is verified, remove all `[group-debug]` lines. Leave the test (with the user's actual fixture values) as the regression guard.

## Hard requirement

**Do NOT write another speculative fix without the logs from step 2.** Two iterations of "I think it's X" have been wrong. If the implementer cannot get the user to capture the logs (because the dev tools workflow isn't accessible), surface that BEFORE writing code — don't guess again.

## Acceptance Criteria

- [ ] Logging committed and pushed for the user to run.
- [ ] User-captured `[group-debug]` log lines pasted into this task's implementation notes — showing the exact stage where data is lost.
- [ ] Fix targets the stage identified in the logs, NOT a hypothesis.
- [ ] New regression test uses the user's exact logged values (perspective view, active view id, entity type, scope chain). The test FAILS with the current code at HEAD + those values, PASSES after the fix.
- [ ] Logging removed in the same commit as the fix, OR a follow-up commit clearly tagged "remove [group-debug] tracing."
- [ ] User confirms the popover now shows fields in their live app.
- [ ] Existing iter-1 and iter-2 tests still pass.

## Tests

- [ ] Run: `cargo test -p swissarmyhammer-kanban` — green.
- [ ] Run: `pnpm -C kanban-app/ui test command-popover perspective-tab-bar` — green.
- [ ] The new test uses real `KanbanContext::open` + real builtin YAML + the user's actual `view_id`/`view`/`active_view_id` values from the logs.

## Workflow

1. Add the tracing/console.log per step 1.
2. Push.
3. Wait for user to paste logs.
4. Diagnose from logs.
5. Write failing test reproducing the logged scenario.
6. Fix.
7. Remove tracing.
8. Verify with user.

**No step may be skipped. No fix may precede step 3's data.** #command-driven-ui #bug #iter3