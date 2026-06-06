---
assignees:
- claude-code
position_column: todo
position_ordinal: ce80
title: Navigation menu does not list the motion commands
---
REOPENED 2026-06-06 — prior fix was WRONG and discarded. The native Navigation menu does not list the motion/nav commands (nav.up/down/left/right/first/last/jump/drillIn/drillOut), and the command palette is also missing from the OS menu.

## OWNER CORRECTION (authoritative — supersedes all prior notes)
The CommandService MUST specify OS-menu placement. The plugins/command-service exist to REPLACE the builtin YAML (`compose_registry!` / `nav.yaml`). The OS menu must be built FROM the CommandService catalogue, and each command's `menu` (MenuPlacement) must be carried THROUGH the service catalogue.

The discarded approach (do NOT repeat): a prior attempt made `state.rs::sync_commands_registry_from_metadata` OVERLAY the builtin `compose_registry!` YAML on top of the CommandService snapshot (new `compose_facade_registry` / `overlay_defs`) to re-inject the nav menu metadata. That is inconsistent with the architecture — it props up the YAML the service is meant to replace. All of that code was reverted to HEAD.

## Correct goal
Make commands available on the OS menu via the CommandService — including:
- Navigation (the nine nav.* motion commands), AND
- the command palette opener (currently absent from the OS menu entirely).
The service catalogue is the single source of truth; the menu reads from it. If nav/focus command metadata (incl. `menu` placement) does not currently reach the service catalogue, the fix is to make it reach the service — not to overlay YAML.

## Next step
Re-investigate (architecturally-correct lens): how command `menu` placement is supposed to flow into the CommandService catalogue and out to `apps/kanban-app/src/menu.rs`; why nav.* (and the palette command) menu placement does not reach the service catalogue today; and the minimal change so the service carries it. TDD, RED first. This card is now part of the broader "commands on the OS menu" effort the owner is driving next.