---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffaa80
title: Enter on the already-active perspective tab should drill into the name editor, not re-select
---
## What

LIVE UX refinement (user-observed 2026-06-13), follow-on from the selection work (01KTYQY0ZB62KHN6BPK3FBMBD7): selecting a perspective works. But pressing **Enter again on the perspective that is already active/selected** just re-selects it (a no-op) — the user expects it to **drill into the name/caption editor** (inline rename).

This is the standard tab/drill idiom: Enter on an unfocused-or-inactive item = select/activate; Enter on the already-active item = drill in (edit). Today Enter is hard-wired to `perspective.switch` regardless of whether the tab is already the active perspective.

## Expected

- Enter on a tab that is NOT the active perspective → activate it (current behavior, keep).
- Enter on the tab that IS already the active perspective → arm inline rename (drill into the caption editor) — the same rename machinery F2 / double-click / the + button's post-create arming already use (`startRename` / `InlineRenameEditor` / `useArmRenameOnArrival` reuse — do NOT build new).
- F2 and the context-menu Rename row stay as additional explicit paths.

## Design / investigate

- Where does Enter resolve on a focused tab today? The selection card mapped the positional `nav.drillIn` shadow on `ScopedPerspectiveTab` → `perspective.switch`. The drill idiom means: the tab's Enter handler must branch on "am I already active?" — if active, arm rename instead of dispatching switch. Determine the cleanest seam:
  - Is "active perspective" known at the tab (perspective-context `activePerspective` / per-window `active_perspective_id`)? The tab already renders an active state, so the bit is available client-side.
  - Prefer the branch live in the tab's Enter/drill handler (presentation-layer routing of a key to either the switch command OR the local rename-arm), NOT a new backend command. Switch stays a command dispatch; rename-arm is the existing local presentation gesture (consistent with metadata-driven-ui + the + button precedent).
- Mind the nav.drillIn semantics generally: this is specifically the perspective-tab surface; don't change global drill behavior.
- Keep the + button create→arm-rename flow and the F2/double-click/context-menu rename all green.

## Acceptance Criteria
- [ ] Enter on a non-active tab activates it (no rename)
- [ ] Enter on the already-active tab arms inline rename (caption editor focused), does NOT re-dispatch switch
- [ ] F2, double-click, context-menu Rename still arm rename from any tab
- [ ] + button create→arm-rename still works
- [ ] Escape from the Enter-armed rename keeps the name (consistent with the established rename-cancel semantics)

## Tests
- [ ] vitest red-first: Enter on active tab arms rename (red: dispatches switch / no editor); Enter on inactive tab dispatches switch and does NOT arm rename
- [ ] the existing selection + button + F2 rename suites stay green
- [ ] tsc + touched suites green

## Constraints
- NO whole-workspace builds; no kanban-app crate compile. Never touch .kanban/actors/wballard.jsonl or crates/swissarmyhammer-ui-state unless required (note if so).
- Reuse existing rename + switch machinery; presentation-layer branch only.

## Workflow
- /tdd.