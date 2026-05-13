---
assignees:
- claude-code
position_column: todo
position_ordinal: ab80
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

- [ ] On a single-enum-param command (e.g. `perspective.group` post-resolver), clicking a field option dispatches the command and closes the popover. No Submit button is rendered for this case.
- [ ] The `(none)` clear-command sentinel is rendered as the first menu item when `clear_command` is set; clicking it dispatches the redirect and closes the popover. Still one click.
- [ ] On a multi-enum-param command (e.g. `perspective.sort.set` with field + direction), the current Submit pattern is preserved — picking a single option does NOT dispatch alone.
- [ ] Keyboard navigation works: arrow keys move focus, Enter activates, Esc closes.
- [ ] Spatial-nav moniker for each option is deterministic and unique within the popover.

## Tests

- [ ] `command-popover.test.tsx`: new test `single_enum_param_click_dispatches_and_closes` — render with a single enum param + 2 options, click the second option, assert `onCommit` was called with `{ paramName: "option-2-value" }` AND the popover is closed.
- [ ] `command-popover.test.tsx`: new test `multi_enum_param_click_does_not_dispatch_without_submit` — render with two enum params, click one option in each, assert `onCommit` was NOT called until Submit is pressed.
- [ ] `command-popover.test.tsx`: new test `clear_command_sentinel_click_dispatches_redirect` — render with `clear_command` set, click `(none)`, assert `onCommit` was called with the redirect target (or the parent dispatcher receives the right thing per the `clear_command` machinery).
- [ ] `perspective-tab-bar.group-migration.test.tsx`: update the existing dispatch tests to reflect the one-click flow (no Submit needed for the Group case).
- [ ] Run: `pnpm -C kanban-app/ui test command-popover perspective-tab-bar.group-migration` — green.

## Workflow

- Use `/tdd` — write the new single-click test first; let it fail (Submit button still required); refactor `<CommandPopover>`; watch it pass.
- **Do NOT change the multi-param flow** as part of this task. The Sort migration (`01KRE21GJMPP289N1HSTMJG5HE`) will inherit the new pattern when it lands; verify its tests still pin the Submit requirement for multi-param.
- Keep the menu rendering simple — a `<ul><li><button>…</button></li></ul>` (or similar) is fine. No need for Radix Menu primitives unless the spatial-nav requirements demand it. #command-driven-ui #ux