---
assignees:
- claude-code
position_column: todo
position_ordinal: c280
project: task-card-fields
title: Pick smarter icons for created and completed date fields
---
## What

The `created` and `completed` date fields currently use generic action-style icons that can be confused with UI actions (the "+" and "checkmark" that appear on add buttons and checkbox toggles).

**Current:**
- `swissarmyhammer-kanban/builtin/definitions/created.yaml` → `icon: plus-circle`
- `swissarmyhammer-kanban/builtin/definitions/completed.yaml` → `icon: check-circle`

**Recommended (calendar-themed, unambiguous for date fields):**
- `created.yaml` → `icon: calendar-plus` — reads as "added on this date"
- `completed.yaml` → `icon: calendar-check` — reads as "completed on this date"

Both icons exist in lucide-react (v0.575.0) and are supported by the unrestricted icon resolver at `kanban-app/ui/src/components/entity-icon.tsx`, which converts kebab-case to PascalCase (`calendar-plus` → `CalendarPlus`) and looks up the lucide export dynamically. No allowlist to update.

Leaving the other four date-field icons untouched (user only requested these two):
- `due`: `calendar`
- `scheduled`: `clock`
- `started`: `play`
- `updated`: `refresh-cw`

### Files to modify

- `swissarmyhammer-kanban/builtin/definitions/created.yaml` — change `icon: plus-circle` to `icon: calendar-plus`
- `swissarmyhammer-kanban/builtin/definitions/completed.yaml` — change `icon: check-circle` to `icon: calendar-check`

Implementer may pick different lucide icon names if they're clearly more semantic, but the icons must:
- Be valid lucide-react icon names (v0.575.0)
- Unambiguously convey "when created" / "when completed" (not generic action icons)
- Visually fit alongside the other date-field icons

## Acceptance Criteria
- [ ] `created.yaml` icon is no longer `plus-circle`; new choice unambiguously reads as a creation timestamp
- [ ] `completed.yaml` icon is no longer `check-circle`; new choice unambiguously reads as a completion timestamp
- [ ] Both chosen icons render correctly via `entity-icon.tsx` (no fallback to `LayoutGrid`)
- [ ] Visual check: icons are distinct from the "+ Add" button icon and from checkbox toggle icons used elsewhere in the app

## Tests
- [ ] `cargo test -p swissarmyhammer-kanban` — passes (builtin field YAML parses)
- [ ] `cargo test -p swissarmyhammer-fields` — passes
- [ ] Manual visual check: run `cd kanban-app && bun run tauri dev`, open a task, confirm the `created` and `completed` field icons render and read as dates rather than actions

## Workflow
- Use `/tdd` — if test assertions reference specific icon strings, update those first; otherwise this is pure YAML data change with a manual visual check.

#task-dates