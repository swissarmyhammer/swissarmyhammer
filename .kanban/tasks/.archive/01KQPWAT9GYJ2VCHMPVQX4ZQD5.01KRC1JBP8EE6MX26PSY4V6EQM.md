---
assignees:
- claude-code
attachments:
- 01KQSDH59T9CTS8X40MPN62N6Z-01KQSDH59M1ANQK3086NA8XCDX-image-j9jBODrxZuGJaiYxWB9gLwDmHggV83.png
position_column: todo
position_ordinal: 7d80
title: Perspectives are scoped by view kind, not view id — all grid views share one pool
---
## Status

**Expanded into four sub-tasks by `/plan` on 2026-05-11. This placeholder is archived — do not implement directly.**

The work is now broken into:

1. `01KRC1C93CD73746F4C0Q2PP86` — Add `view_id` field to `Perspective` with legacy-compat loader (foundational data shape).
2. `01KRC1DRWA3PFC7NFX4WVF3DD8` — Switch backend perspective filters to `view_id` with kind fallback (depends on #1).
3. `01KRC1F2D259GQDN83M1YVPX0R` — Switch frontend perspective tab bar filter to `view_id` with kind fallback (depends on #1; can land in parallel with #2).
4. `01KRC1GGW4SQH6QEDN34ZERQD9` — Migrate existing perspective YAMLs to carry `view_id` where unambiguous (depends on #2 and #3).

The acceptance criteria, file inventory, and test plan from this placeholder are distributed across those four tasks. Compatibility decision recorded in #1: `view_id: None` ⇒ legacy shared-by-kind fallback. Existing YAMLs are not rewritten on read — migration is opt-in via the next save (see #4).

---

## Original placeholder description (kept for reference)

### What

Reported behavior: switching between two grid-kind views (e.g. a tasks grid and a tags grid) shows the **same** perspective tabs in the formula bar. Perspectives saved while one grid is active appear in every other grid view of the same kind.

### Root cause

`Perspective.view` stores the view **kind** string (`"board"`, `"grid"`), not the view's id. Every consumer filters by kind:

- Frontend: `kanban-app/ui/src/components/perspective-tab-bar.tsx:161–165`
- Backend: `swissarmyhammer-kanban/src/commands/perspective_commands.rs` filters by `view_kind` at ~10 sites.
- Backend: `swissarmyhammer-kanban/src/dynamic_sources.rs:113` — `gather_perspectives(view_kind)`.
- Data model: `swissarmyhammer-kanban/src/perspective/add.rs` — `AddPerspective::new(name, view)` where `view` is a kind.
- Type: `kanban-app/ui/src/types/kanban.ts:55` — `PerspectiveDef.view: string` documented as the view kind.
- On-disk: `.kanban/perspectives/*.yaml` files store the kind string in their `view:` field.