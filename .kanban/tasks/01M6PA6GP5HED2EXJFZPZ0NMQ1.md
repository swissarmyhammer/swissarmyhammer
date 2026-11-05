---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6pa75y996kvt9q3v4z53d87
  text: |-
    ### finish iteration 1 — stuck
    - implement: stuck — no edit/shell tools in this session or its subagents (implementer and general-purpose both lacked Edit/Bash)
    - test/commit/review: not run
    - next: re-run `/finish ^pz0nmq1` from a session with Edit + Bash
  timestamp: 2026-11-05T16:56:04.809061+00:00
- actor: claude-code
  id: 01m6pajywfssvk563615z8tawc
  text: |-
    Implementation landed. `KanbanTool::execute` now returns only the operations' own results. Removed the helper block (`task_to_plan_entry`, `read_task_page`, `LIST_TASKS_*`, `list_all_tasks_for_plan`, `build_plan_data`, `attach_plan`, `TASK_MODIFYING_OPERATIONS`, `is_task_modifying_operation`), the PlanEntry* import, and the now-unused `ListTasks`/`MAX_PAGE_SIZE`/`Execute`/`Noun`/`Verb` imports. Replaced the module doc's "Plan Notifications" section with "Response Scope" and updated the `execute`/`run_op` docs. Deleted every `_plan` test and both `attach_plan` tests. Kept `test_delete_task`, `init_test_board`, `extract_task_id`, and `plan_notifications.rs`/`PlanSender`.
    Regression tests (TDD): `test_add_task_reply_names_no_other_card` and `test_move_task_reply_names_no_other_card` seed `BYSTANDER_CARD_COUNT` = 31 cards. Each asserts the reply has no `_plan` and contains no bystander id or title. RED before the fix: both failed, and the response carried `_plan.entries` naming every bystander. GREEN after the fix.
    CLI: removed `ack_without_plan` and the `_plan` title assertion from `kanban_cli_tests.rs`. The ack is now asserted on stdout directly. serve.rs: the doc comment now describes "Response scope" and no longer mentions `build_plan_data`.
    Note: the running MCP server still uses the old binary, so writes stay huge until sah is rebuilt or reinstalled.

    ### implement — changed
    - evidence: 3 files — crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs, apps/swissarmyhammer-cli/tests/kanban_cli_tests.rs, apps/kanban-cli/src/commands/serve.rs. `cargo test -p swissarmyhammer-tools kanban`: 73 lib passed, plus 2 in another target, 0 failed. `cargo test -p swissarmyhammer-cli --test kanban_cli_tests`: 16 passed. `cargo clippy -p swissarmyhammer-tools -p swissarmyhammer-cli -p kanban-cli --all-targets -- -D warnings`: clean. `cargo fmt --check`: clean.
    - next: /review
  timestamp: 2026-11-05T17:02:30.799653+00:00
- actor: claude-code
  id: 01m6paz3mjrefzcwyerbdy16d7
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (fcccdc6c7) — 0 findings, 0 confirmed, 0 refuted; 7 validator runs attempted, 0 failed; 3 files reviewed, 2 .kanban files excluded by .reviewignore
    - next: none — moved to done
  timestamp: 2026-11-05T17:09:08.882672+00:00
- actor: claude-code
  id: 01m6pazt2mce421qd38xnnrq0j
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 3 files (kanban/mod.rs, kanban_cli_tests.rs, serve.rs)
    - test: green — cargo nextest -p swissarmyhammer-tools -p swissarmyhammer-cli -p kanban-cli, 1746 passed; clippy -D warnings clean
    - commit: fcccdc6c7
    - review: clean — 0 findings; card moved to done
  timestamp: 2026-11-05T17:09:31.860067+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffe880
title: 'Kanban write ops must not return the whole board: remove `_plan` attachment'
---
## Problem
Every task-modifying kanban MCP call (add/update/delete/move/complete/assign/unassign/tag/untag task, add/update/delete comment) attaches `_plan` listing EVERY card on the board (`list_all_tasks_for_plan` pages through all cards incl. done). On this repo's board that's ~180K chars per write, overflowing agents' tool results and pushing them to grep `.kanban/tasks/*.md`. Regressed when 01KYTC765 fixed empty `_plan.entries`. Nothing consumes `_plan` outside tests; `kanban-cli serve` already omits it.

Rule: a kanban call never returns the whole board unless the caller explicitly asked (`get board`, `list tasks`, ...).

## Changes
- `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`
  - drop `PlanEntry`/`PlanEntryPriority`/`PlanEntryStatus` import (~L17)
  - delete helper block ~L95–313 (`task_to_plan_entry` … `is_task_modifying_operation`, incl. `read_task_page`, `LIST_TASKS_*`, `list_all_tasks_for_plan`, `build_plan_data`, `attach_plan`, `TASK_MODIFYING_OPERATIONS`)
  - `execute`: remove plan tracking/attach (~L615–657), fix doc comment (~L586), module doc (~L6–14), `run_op` doc (~L722)
  - delete `_plan` tests ~L1205–1701 (keep `test_delete_task` and helpers `init_test_board`, `extract_task_id`) and the two `attach_plan` tests ~L3679/3694
- `apps/swissarmyhammer-cli/tests/kanban_cli_tests.rs`: remove `ack_without_plan` (L25–45, used at L416) and `_plan` title assertion (L425–431)
- `apps/kanban-cli/src/commands/serve.rs`: update doc comment L154–160
- Keep `plan_notifications.rs` / `PlanSender` (still used in tool_registry/context.rs, re-exported mcp/mod.rs:86)

## Acceptance
- [ ] Regression test: board seeded with 30+ cards; `add task` and `move task` responses have no `_plan` and contain no other card ids/titles
- [ ] `cargo test -p swissarmyhammer-tools kanban` passes
- [ ] swissarmyhammer-cli `kanban_cli_tests` pass
- [ ] clippy clean on touched crates #bug