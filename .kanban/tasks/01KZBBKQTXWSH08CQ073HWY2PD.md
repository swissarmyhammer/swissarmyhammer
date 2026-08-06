---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8c80
title: Resolve project types on the hook and list/dump validators surfaces
---
Follow-up to ^ygt2rre. Its research note found: `lib.rs::match_rules` (the hook surface) and the `list validators` / `dump validators` ops have no workspace root. They resolve no project types. A `project_types`-keyed rule fails closed there and silently never matches.

Why this matters now: the implement skill calls `dump validators` for rules-up-front. Tool rules key on `project_types` (^q4909tf, ^b01gtzg). Without this fix, the rule dump omits every tool rule — the implementer never sees them, and only the review engine enforces them. That recreates the surprise-findings problem this whole project exists to remove.

Work:
- Thread a workspace root into `match_rules` and the `list/dump validators` ops. Resolve once per call with the same `detected_project_type_keys` helper `scope.rs` uses (^ygt2rre landed it).
- Resolve the root from the session working dir, never `std::env::current_dir()`.
- Fail-closed stays the behavior when no root is available.

Acceptance:
- `dump validators` from a rust workspace includes a `project_types: [rust]` keyed rule; from a non-rust workspace it does not.
- The hook surface matches a project_types-keyed validator in a matching workspace.

#tool-validators