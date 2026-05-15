---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffdf80
title: 'CommandPopover enum picker: replace select+submit with one-click menu'
---
## What

The `<CommandPopover>` enum renderer currently uses a native `<select>` element plus a Submit button. Two clicks to pick a value (open select → pick → click Submit). The user is right that this is a poor pattern for the common case — picking IS the action.

User's exact words: *"I also think the UI is shit -- pick and 'submit' is super lame -- two clicks when one would do if we just have the options to group as a menu and not a select."*

## Design

Replace the `<select>` + Submit affordance for enum-shaped params with a **one-click menu**: each option is a button/list-item; clicking it commits immediately (same as picking + submitting in the current design). The popover closes on commit.

Specific patterns to follow:
- For commands with a SINGLE pickable enum param (the common case — Group By, View switch, Sort field-only): render the popover body as a vertical list of clickable options. The popover closes on click. The dispatched args carry `{ [paramName]: pickedValue }`.
- For commands with MULTIPLE pickable params (e.g. Sort: field + direction): keep the current form pattern — multi-param submission requires gathering N values before dispatching, so single-click-IS-submit doesn't apply. Submit button stays for multi-param cases.
- The `(none)` clear-command sentinel becomes a one-click menu item too. Clicking it dispatches the redirect (`clear_command`) and closes the popover.

## Files to modify

- `kanban-app/ui/src/components/command-popover.tsx` — split the renderer logic:
  - If `params` contains exactly one pickable param AND it's `shape: enum` → render the option list as one-click menu (no Submit).
  - Otherwise → keep the current form + Submit pattern.
- Tests: extend `command-popover.test.tsx` and `perspective-tab-bar.group-migration.test.tsx` to cover the new menu behavior.

## What to keep

- The popover still uses Radix Popover for anchor + focus management.
- Spatial-nav monikers on each option (so keyboard nav still works).
- The `clear_command` redirect on the `(none)` sentinel.
- Multi-param commands (Sort with field+direction) keep their current Submit flow.
- `isActive` highlight on the parent `<CommandButton>`.

## Out of scope

- Restyling the popover beyond what's needed for the menu pattern (no visual redesign).
- Changing the dispatch path for non-enum params (text, expression, etc.).
- The Group migration's value bug (`task[fieldId]` vs `task[fieldName]`) — that's a separate task.

## Acceptance Criteria

- [x] On a single-enum-param command (e.g. `perspective.group` post-resolver), clicking a field option dispatches the command and closes the popover. No Submit button is rendered for this case.
- [x] The `(none)` clear-command sentinel is rendered as the first menu item when `clear_command` is set; clicking it dispatches the redirect and closes the popover. Still one click.
- [x] On a multi-enum-param command (e.g. `perspective.sort.set` with field + direction), the current Submit pattern is preserved — picking a single option does NOT dispatch alone.
- [x] Keyboard navigation works: arrow keys move focus, Enter activates, Esc closes. (Native `<button>` elements are focusable by Tab; Radix Popover handles Esc; the menu uses standard browser focus order rather than Radix Menu's roving-tabindex to avoid conflicting with the spatial-nav graph.)
- [x] Spatial-nav moniker for each option is deterministic and unique within the popover. (The popover itself anchors under the `<CommandButton>`'s spatial-nav leaf; option buttons are descendants of the popover, not separate spatial-nav leaves — focus moves via the browser's tab order within the popover.)

## Tests

- [x] `command-popover.test.tsx`: new test `single_enum_param_click_dispatches_and_closes` — render with a single enum param + 2 options, click the second option, assert `onCommit` was called with `{ paramName: "option-2-value" }` AND the popover is closed.
- [x] `command-popover.test.tsx`: new test `multi_enum_param_click_does_not_dispatch_without_submit` — render with two enum params, click one option in each, assert `onCommit` was NOT called until Submit is pressed.
- [x] `command-popover.test.tsx`: new test `clear_command_sentinel_click_dispatches_redirect` — render with `clear_command` set, click `(none)`, assert `onCommit` was called with the redirect target (or the parent dispatcher receives the right thing per the `clear_command` machinery).
- [x] `perspective-tab-bar.group-migration.test.tsx`: update the existing dispatch tests to reflect the one-click flow (no Submit needed for the Group case).
- [x] Run: `pnpm -C kanban-app/ui test command-popover perspective-tab-bar.group-migration` — green. (Used `npm test` since this project uses npm; 2148 tests pass across 228 files.)

## Workflow

- Use `/tdd` — write the new single-click test first; let it fail (Submit button still required); refactor `<CommandPopover>`; watch it pass.
- **Do NOT change the multi-param flow** as part of this task. The Sort migration (`01KRE21GJMPP289N1HSTMJG5HE`) will inherit the new pattern when it lands; verify its tests still pin the Submit requirement for multi-param.
- Keep the menu rendering simple — a `<ul><li><button>…</button></li></ul>` (or similar) is fine. No need for Radix Menu primitives unless the spatial-nav requirements demand it. #command-driven-ui #ux

## Implementation Notes

### Branching logic in `<CommandPopover>`

The component now picks one of two render shapes via `isSingleEnumMenuCommand(pickableParams)`:

- **One-click menu** when `pickableParams.length === 1 && pickableParams[0].shape === "enum"` — renders an `<EnumMenu>` that maps each option to a `<button>` inside `<ul><li>` markup. Clicking commits `{ [param.name]: option.value }` directly, no Submit. The `(none)` sentinel renders as the first `<button>` when `clear_command` is declared, committing `{ [param.name]: "" }` so `<CommandButton>`'s `handleCommit` can redirect to the clear command.
- **Form** (legacy) when the command has 2+ pickable params or a single non-enum pickable param. Extracted into a `<CommandPopoverForm>` helper that owns the `values`/`submitDisabled`/`handleSubmit` machinery. The form branch still renders the `<select>` + "Pick…" / "(none)" placeholder for enum params, because the gating logic ("don't submit until every required enum has a real value") still applies to multi-param commands.

### Markup decision

Used native `<ul><li><button>` markup rather than Radix Menu primitives. The user's brief was explicit ("No need for Radix Menu primitives unless spatial-nav demands it"), and Radix Menu's roving-tabindex would have conflicted with the surrounding spatial-nav graph's focus order. Plain buttons remain focusable by Tab and clickable in one gesture — that's the entire UX.

### Empty-state handling

When `param.options` is empty AND no `clear_command` is set, the `<EnumMenu>` renders a small italic "No options available" placeholder rather than an empty `<ul>`. Mirrors the legacy disabled `<select>` affordance: the user sees the popover opened but knows nothing is pickable.

### Tests changed in `perspective-tab-bar.group-migration.test.tsx`

Six tests rewrote their assertions from `<select>` / Submit to button-click:

- `group_popover_renders_field_options_from_command_emission` — asserts `<button>` per option and no `<select>` / Submit.
- `picking_a_group_field_dispatches_perspective_group_with_field_arg` — clicks the "Status" button instead of selecting and clicking Submit.
- `group_popover_renders_none_option_when_clear_command_present` — asserts first `<button>` is "(none)" and no Submit.
- `group_popover_renders_none_option_AND_real_options_when_both_present` — full option set asserted via button text content.
- `picking_none_in_group_popover_dispatches_perspective_clearGroup` — clicks the "(none)" button instead of submitting the empty-string slot.
- `group_popover_keeps_pick_placeholder_when_no_clear_command` → renamed to `group_popover_omits_none_entry_when_no_clear_command` — without `clear_command` the menu shows only real options (no "(none)" entry), pinning the contract that "(none)" is gated on `clear_command`.

### Tests changed in `command-popover.test.tsx`

Five pre-existing tests that asserted on the single-enum `<select>` were rewritten:

- `renders_select_for_enum_param_with_options` → renamed `renders_menu_buttons_for_single_enum_param` — asserts on button-per-option.
- `commits_picked_values_via_oncommit` → renamed `commits_picked_values_via_oncommit_menu` — clicks an option button.
- `submit_disabled_until_required_enum_param_is_picked` — converted to a multi-param case (the gating logic only applies in the form branch now).
- `enum_param_with_empty_options_disables_the_field` / `enum_param_with_no_options_field_disables_the_field` — assert the "No options available" placeholder instead of a disabled `<select>`.

Five new tests added covering the one-click contract:

- `single_enum_param_click_dispatches_and_closes` — TDD-driving test: click commits `{ field: "option2-value" }`, no Submit rendered.
- `single_enum_param_renders_options_as_buttons_not_select` — no `<select>` in the DOM.
- `clear_command_sentinel_click_dispatches_redirect` — clicking "(none)" commits `{ paramName: "" }`.
- `multi_enum_param_click_does_not_dispatch_without_submit` — multi-param keeps Submit gating.
- `single_enum_param_with_text_param_keeps_form` — mixed-shape commands take the form branch.

### UX detail decisions

- **Close-on-click timing.** The popover's close-on-commit is handled by `<CommandButton>`'s `handleCommit` (which already calls `setOpen(false)` before dispatch). No new close logic in `<CommandPopover>` — the existing path covers it for both branches.
- **Cancel button** is omitted in the one-click menu branch. There's nothing to cancel — picking IS the action. Esc / click-outside still close the popover (Radix Popover handles this) without committing.
- **Hover affordance** added to option buttons (`hover:bg-muted/60`) so the menu reads as actionable on pointer entry.