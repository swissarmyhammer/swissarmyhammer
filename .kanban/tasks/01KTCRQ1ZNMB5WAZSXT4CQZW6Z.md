---
assignees:
- claude-code
position_column: todo
position_ordinal: c580
title: 'Bug: Navigation menu is blank — nav/jump commands missing from native menu'
---
## What
Reported by user: the **Navigation** menu in the native macOS menu bar is blank. It should list the nine `nav.*` commands (directional `nav.up/down/left/right`, `nav.first/last`, `nav.drillIn/drillOut`, and `nav.jump`).

The menu is built in `apps/kanban-app/src/menu.rs`:
- `build_menu_from_commands` calls `build_grouped_submenu(app, "Navigation", menus.get("Navigation"), …)` (menu.rs:58).
- `collect_menu_entries` only emits an entry for a command if `cmd.menu` placement is present and `placement.path` is non-empty (menu.rs:88–107).

The in-process unit test `navigation_submenu_contains_all_nine_nav_commands` (menu.rs:884) passes against a registry composed with `compose_registry![swissarmyhammer_commands, swissarmyhammer_focus, swissarmyhammer_kanban]`. So the YAML placement is correct in isolation — meaning the **runtime** registry handed to `build_menu_from_commands` likely does NOT include the `swissarmyhammer-focus` `nav.*` commands, or the menu is never rebuilt after the focus crate's commands are composed.

Investigate the runtime registry composition (where `state.commands_registry` is populated — `apps/kanban-app/src/state.rs` / `main.rs`) and confirm the focus crate is composed into the live registry that `rebuild_menu_inner` reads. Compare against the test's `compose_registry!` list.

NOTE: likely shares a root cause with the command-palette-launch bug and the jump-to-inspect bug — all three point at `nav.*`/focus commands not being surfaced at runtime. Cross-check before fixing in isolation.

## Acceptance Criteria
- [ ] The native Navigation submenu renders all nine `nav.*` items at runtime (not blank).
- [ ] Root cause identified: whether the live `commands_registry` omits the focus-crate commands, or the menu isn't rebuilt after composition.

## Tests
- [ ] Add/extend a test that builds the menu from the SAME registry the app composes at runtime (the production composition path), not a hand-built `compose_registry!` in the test — assert the "Navigation" key has 9 entries. Co-locate with the existing tests in `apps/kanban-app/src/menu.rs`.
- [ ] Regression test that fails before the fix and passes after.
- [ ] `cargo test -p kanban-app menu` green.

## Workflow
- Use `/tdd` — write the failing test first, then fix. #bug