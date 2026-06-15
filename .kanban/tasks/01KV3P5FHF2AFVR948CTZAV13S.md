---
assignees:
- claude-code
position_column: todo
position_ordinal: fa80
project: builtin-commands
title: Fix 2 failing field-pill spatial-nav browser tests (right/escape don't dispatch nav commands)
---
## Problem

Two browser tests in `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx` fail (the other 4 in the file pass):

- **`right_from_first_pill_lands_on_second_pill`** (L607): after seeding the first tag pill (`tag:tag-bug`) as focused inside a field zone, firing `ArrowRight` should make the global `nav.right` command dispatch `spatial_navigate({ focusedFq: bugPill.fq, direction: "right" })`. **Asserts `navCalls.length === 1`; actual `0`.**
- **`escape_from_pill_drills_back_to_field_zone`** (L690): with the bug pill focused, firing `Escape` should make `nav.drillOut` dispatch `spatial_drill_out(pillFq)` and forward the kernel-returned field-zone moniker through `FocusActions.setFocus`. **Asserts `drillOutCalls.length === 1`; actual `0`.**

Zero dispatches means the keydown (`ArrowRight` / `Escape`) is no longer resolving through the keymap to the `nav.right` / `nav.drillOut` command closures **when focus is on a pill nested inside a field zone**. The "Enter on a field zone" cases in the same file still pass, so the field-zone scope itself works — the break is specific to the spatial-nav (arrow/escape) bindings reaching a pill leaf.

## Root-cause suspect

The file's last touch was commit `6dc3b7d07` *"refactor(commands)!: rename ui.\* command ids to app.\*; fold ui-commands into app-shell-commands"* (preceded by `a13f720ae` *"move grid.\* and field/pressable commands to builtin plugins"*). Strong suspicion: the keymap binding / command registration that routes `ArrowRight`→`nav.right` and `Escape`→`nav.drillOut` for a pill leaf inside the `ui:field` marker scope was lost or mis-wired in that rename/move (e.g. a stale `ui:*` id in the keymap chain walk, or the nav binding no longer applying within the field-zone marker subtree). Confirmed pre-existing (independent of the recently-landed `field.tsx` "Edit Field" work — reverting `field.tsx` to HEAD reproduces both failures identically).

## What

- Bisect to confirm whether `6dc3b7d07` / `a13f720ae` introduced the regression: `git stash` not needed — run the test at those SHAs (`git -C . show`/checkout in a scratch clone, or inspect the diff) to find where the `nav.right` / `nav.drillOut` binding stopped reaching a pill-in-field leaf.
- Fix the root cause in the command/keymap wiring (likely in `builtin/plugins/nav-commands/index.ts`, `builtin/plugins/app-shell-commands/`, or the frontend keymap chain-walk / `CommandScopeProvider` for the `ui:field` marker — `apps/kanban-app/ui/src/lib/command-scope.tsx` and the field-zone scope in `apps/kanban-app/ui/src/components/fields/field.tsx`). Do NOT weaken the test assertions or convert them to no-ops.
- The fix must restore: arrow-key spatial nav and Escape drill-out dispatching the correct `spatial_navigate` / `spatial_drill_out` MCP calls for a focused pill leaf nested in a field zone.

Note: as of the latest commits the broader `swissarmyhammer-command-service` suite is green; this is an isolated frontend test gap.

## Acceptance Criteria
- [ ] `right_from_first_pill_lands_on_second_pill` passes: one `spatial_navigate` call with `{ focusedFq: bugPill.fq, direction: "right" }`.
- [ ] `escape_from_pill_drills_back_to_field_zone` passes: one `spatial_drill_out` call with `{ fq: bugPill.fq }`, and the returned field-zone moniker is forwarded via `setFocus`/`spatial_focus`.
- [ ] The other 4 tests in the file still pass (no regression to the Enter/edit/drill-in cases).
- [ ] Assertions are unchanged (or strengthened) — the fix is in production wiring, not the test.

## Tests
- [ ] `cd apps/kanban-app/ui && npx vitest run src/components/entity-inspector.field-enter-drill.browser.test.tsx` → all 6 pass (currently 2 fail / 4 pass).
- [ ] If the root cause is a keymap/binding wiring bug shared by other surfaces, add/extend a focused unit test at the wiring layer (e.g. the keymap chain-walk for a marker-scoped command) so the regression can't silently return.
- [ ] Run the surrounding frontend suite for the touched area (`npx vitest run src/components/fields src/components/entity-inspector`) to confirm no collateral breakage.

## Workflow
- Use `/tdd` — the two failing tests already encode the expected behavior; run them red first, root-cause the binding break, then fix the wiring until both (and the other 4) are green. #Test-Failure #navigation #frontend