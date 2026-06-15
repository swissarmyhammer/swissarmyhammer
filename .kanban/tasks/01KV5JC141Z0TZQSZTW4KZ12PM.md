---
assignees:
- claude-code
position_column: todo
position_ordinal: fb80
project: builtin-commands
title: 'Fix: Enter on the already-selected perspective tab doesn''t drill into the caption editor'
---
## Problem

Pressing Enter on an already-selected (active) perspective tab should drill into the inline **caption editor** (edit the perspective name) — the established "tab/drill idiom": Enter on an unselected tab *selects* it; Enter on the already-selected tab *drills in to edit*. In the real app, the drill-in does nothing — the caption editor never opens.

The select-vs-drill logic already exists as a **single branching handler** and we are keeping that shape (approach chosen with the user — fix the branch, do NOT re-architect dispatch into a pass-along chain). In `apps/kanban-app/ui/src/components/perspective-tab-bar.tsx`, `ScopedPerspectiveTab` (~L1042) registers a positional shadow of the global `nav.drillIn` (Enter):

```ts
id: "nav.drillIn",
execute: async () => {
  if (isActive) { triggerStartRename(perspective.id); return; }   // drill in → edit caption  ← broken in prod
  await dispatchPerspectiveSwitch({ args: { perspective_id: perspective.id } }); // select
},
```

`isActive = activePerspectiveId === perspective.id`. The `isActive` branch arms the existing inline rename machinery (`triggerStartRename` → module-level `onStartRename` subscribers → `startRename(id)` → `renamingId` state → `PerspectiveTab` renders the `TextEditor`).

## Smell: a green test that doesn't reflect production

There is already a PASSING spatial test asserting this works — `apps/kanban-app/ui/src/components/perspective-tab-bar.activate-and-rename.spatial.test.tsx` test #1 ("Enter on the focused already-active perspective tab arms inline rename …", asserts `renameEditor` is not null). So the bug is NOT in the harness-focused-tab path the test exercises; it's in the **production focus/scope condition** the test doesn't reproduce. Per the `real-path-tests-not-mocks` principle, the regression test for this fix must drive the actual production path, not the path that already passes.

## Root-cause directions (investigate, then fix the smallest correct thing)

- [ ] **Does Enter even reach the tab's `nav.drillIn` shadow in production?** When a perspective is active/selected, confirm where spatial focus actually sits. Hypothesis: focus is on the board/content (or a non-tab leaf), so Enter resolves to the **global** `nav.drillIn` (drills into content) and never hits the tab's scope-local shadow. The shadow only wins while the `perspective:<id>` tab scope is the focused chain (`command-scope.tsx` shadow rule: inner scope wins only when it's in the focused chain).
- [ ] **Is `isActive` stale?** `activePerspectiveId` is a prop; after a `perspective.switch` the UI-state event may not have propagated, so a second Enter could still see `isActive === false` and re-dispatch switch instead of drilling. (The F2 handler already works around exactly this by passing an explicit id.) Confirm whether the "already-selected" case the user hits has a fresh or stale `isActive`.
- [ ] **Does `triggerStartRename` reach a mounted subscriber and focus the editor?** Verify `onStartRename` has a live subscriber for the active bar, `renamingId` is set, the `TextEditor` mounts (`renamingId === perspective.id`), and DOM focus lands in it. (Note the existing `console.warn("[rename] …")` instrumentation in `usePerspectiveRename` — use it / the OS log to trace, do not ask the user to check the browser console.)

## What

- Root-cause which of the above breaks the production drill-in, and fix it in `apps/kanban-app/ui/src/components/perspective-tab-bar.tsx`, keeping the single branching `nav.drillIn` shadow (inactive → `perspective.switch`; already-active → arm caption editor). Likely fixes, depending on root cause: ensure the tab scope is the focused chain when a selected perspective's tab is the target of Enter, and/or pass the explicit perspective id and not rely on a possibly-stale `isActive`, and/or repair the `triggerStartRename → startRename → editor focus` path.
- Do NOT add a new backend command or new dispatch primitive; this is presentation-layer routing only (reuse the same caption editor F2 / double-click arm).

## Acceptance Criteria
- [ ] In the production focus path, pressing Enter on the already-selected perspective tab opens the inline caption editor with DOM focus in it (ready to type), reusing the same editor F2/double-click use.
- [ ] Enter on an unselected/inactive perspective tab still selects it (`perspective.switch`) and does NOT open the editor.
- [ ] Enter on the already-selected tab does NOT re-dispatch `perspective.switch` (re-selecting the active perspective is a no-op).
- [ ] F2 rename and double-click rename behavior are unchanged.
- [ ] Enter outside the perspective tab scope still drills via the global `nav.drillIn` (no regression to board/content drill-in).

## Tests
- [ ] Add a regression test in `apps/kanban-app/ui/src/components/perspective-tab-bar.activate-and-rename.spatial.test.tsx` (or a sibling) that reproduces the PRODUCTION focus/scope condition under which Enter currently fails to drill — it must fail before the fix and pass after. If the existing green test #1 sets up a focus state that does not match production, the new test must drive the real path (focus as the running app leaves it when a perspective is selected), not re-assert the already-passing path.
- [ ] Keep/verify the existing cases: Enter on inactive tab → `perspective.switch`, no editor; Enter on already-active tab → editor mounts, no re-switch; Enter outside perspective scope → global `nav.drillIn`.
- [ ] `cd apps/kanban-app/ui && npx vitest run src/components/perspective-tab-bar.activate-and-rename.spatial.test.tsx` → all pass (new case red before fix, green after).

## Workflow
- Use `/tdd` — first write the test that reproduces the production failure (Enter on the selected tab does not open the caption editor), confirm it's red, root-cause, then fix the branch until green. #perspectives #commands #bug