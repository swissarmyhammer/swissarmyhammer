---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6bpxkx60kdb7xn0bp9wy901
  text: |-
    ### implement — changed
    - Deleted crates/swissarmyhammer-focus; removed from workspace members and workspace deps (Cargo.toml); removed dev-dependency in swissarmyhammer-kanban/Cargo.toml; removed ARCHITECTURE.md paragraph; fixed doc comment in swissarmyhammer-commands/src/macros.rs.
    - Card fact was wrong: two kanban TEST files used the focus crate (tests/composed_commands_registry.rs, tests/builtin_commands.rs). Removed focus from compose and dropped the 9 nav.* ids; count 72 -> 63.
    - cargo build --workspace: ok. cargo test -p swissarmyhammer-kanban: all pass, 0 failures, no warnings.
    - next: review
  timestamp: 2026-11-01T14:06:24.166399+00:00
- actor: claude-code
  id: 01m6bqac2jr8vpf84cpyj83357
  text: |-
    ### test — green
    - evidence: cargo build --workspace --all-targets: 0 warnings, exit 0. cargo test -p swissarmyhammer-kanban -p swissarmyhammer-commands -p swissarmyhammer-entity -p swissarmyhammer-views -p swissarmyhammer-perspectives -p kanban-cli: 2411 passed, 0 failed, 0 ignored, 0 warnings. First run showed 2 ignored doctests in crates/swissarmyhammer-commands/src/macros.rs (compose_registry, compose_yaml_sources). They were `ignore` fences. They are now runnable examples and pass.
    - next: review
  timestamp: 2026-11-01T14:13:22.130224+00:00
- actor: claude-code
  id: 01m6bqbpqqw7pf189nq3y0acwx
  text: |-
    ### commit — changed
    - evidence: see git log subject "refactor: remove the unused swissarmyhammer-focus crate" (focus crate deleted, Cargo.toml, Cargo.lock, kanban Cargo.toml and 2 tests, commands macros.rs, ARCHITECTURE.md focus hunk only)
    - next: review
  timestamp: 2026-11-01T14:14:05.815679+00:00
- actor: claude-code
  id: 01m6bqhakd6rtbr2hagf3v2x60
  text: |-
    ### review — findings
    - evidence: review sha e721c44e3~1..e721c44e3; 2 findings (2 confirmed, 1 refuted) — crates/swissarmyhammer-kanban/tests/builtin_commands.rs:190, crates/swissarmyhammer-kanban/tests/composed_commands_registry.rs:21
    - next: implementer must fix both items in the dated "Review Findings (2026-11-01 08:14)" section, then re-review.
  timestamp: 2026-11-01T14:17:09.997663+00:00
- actor: claude-code
  id: 01m6bqhx4fvppyw9s4ssrjzabe
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — focus crate deleted, kanban tests updated
    - test: green — 2411 passed, 0 failed, 0 ignored, 0 warnings
    - commit: e721c44e3
    - review: findings — crates/swissarmyhammer-kanban/tests/builtin_commands.rs:190, crates/swissarmyhammer-kanban/tests/composed_commands_registry.rs:21 (literal 63 repeated; use one named constant)
  timestamp: 2026-11-01T14:17:28.975389+00:00
- actor: claude-code
  id: 01m6bqkh9a8fwc1pd7qwcsy4et
  text: |-
    ### implement — changed
    - evidence: 3 files — crates/swissarmyhammer-kanban/tests/common/mod.rs (new, EXPECTED_COMPOSED_COMMAND_COUNT), tests/builtin_commands.rs, tests/composed_commands_registry.rs. Both files use `mod common;`. Also named the bare param count 1 (SINGLE_ID_PARAM_COUNT) and removed stale counts from doc comments. `cargo test -p swissarmyhammer-kanban`: 1401 unit + all integration binaries pass (builtin_commands 5, composed_commands_registry 4), 0 failures, 0 warnings.
    - next: /review
  timestamp: 2026-11-01T14:18:22.378142+00:00
- actor: claude-code
  id: 01m6bqp69rxmynjq2avmjfznkg
  text: |-
    ### test — green
    - evidence: cargo build --workspace --all-targets (exit 0, 0 warnings); cargo test -p swissarmyhammer-kanban -p swissarmyhammer-commands -p swissarmyhammer-entity -p swissarmyhammer-views -p swissarmyhammer-perspectives -p kanban-cli — 2411 passed, 0 failed, 0 ignored, 0 warnings
    - next: review
  timestamp: 2026-11-01T14:19:49.432329+00:00
- actor: claude-code
  id: 01m6bqpsk5qbxnp3fjxwzv2qce
  text: |-
    ### commit — changed
    - evidence: ea9cf3efe test(kanban): name the composed command count in one shared constant. Files: crates/swissarmyhammer-kanban/tests/common/mod.rs (new), tests/builtin_commands.rs, tests/composed_commands_registry.rs.
    - next: review
  timestamp: 2026-11-01T14:20:09.189905+00:00
- actor: claude-code
  id: 01m6bqsw8z8y0nczap40kgvgya
  text: |-
    ### review — clean
    - evidence: review sha ea9cf3efe~1..ea9cf3efe, 3 files reviewed, counts: findings 0, confirmed 0, refuted 0. Prior 2 items (repeated literal 63) are checked.
    - next: task moved to done.
  timestamp: 2026-11-01T14:21:50.239879+00:00
- actor: claude-code
  id: 01m6bqt6v44f10vg9dytbnydan
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — shared EXPECTED_COMPOSED_COMMAND_COUNT in tests/common/mod.rs
    - test: green — 2411 passed, 0 failed, 0 warnings
    - commit: ea9cf3efe
    - review: clean — 0 findings, task moved to done
  timestamp: 2026-11-01T14:22:01.060476+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffd780
title: Remove the unused swissarmyhammer-focus crate
---
The GUI app is gone. The swissarmyhammer-focus crate (spatial navigation) has no users now.

Facts:
- Only swissarmyhammer-kanban lists it as a dependency.
- No source file in swissarmyhammer-kanban uses it (zero files).
- The only other mention is a doc comment in its own lib.rs.

Steps:
- Delete crates/swissarmyhammer-focus.
- Remove it from the workspace members in the root Cargo.toml.
- Remove the dependency line in crates/swissarmyhammer-kanban/Cargo.toml.
- Search for other mentions (docs, CI, README, ARCHITECTURE.md) and remove them.
- Run the build and the tests for the workspace. They must pass. #cleanup #dead-code

## Review Findings (2026-11-01 08:14)

> Scope: `review sha e721c44e3~1..e721c44e3` — reviewed the diffs only — lines this change added or modified. 21 file(s) reviewed, 4 not reviewed.

> 4 file(s) not reviewed — no validator matched:
> - `ARCHITECTURE.md` — no validator matches this file
> - `Cargo.lock` — no validator matches this file
> - `crates/swissarmyhammer-focus/README.md` — no validator matches this file
> - `crates/swissarmyhammer-focus/builtin/commands/nav.yaml` — no validator matches this file

- [x] `crates/swissarmyhammer-kanban/tests/builtin_commands.rs:190` `code-hygiene/magic-numbers` — The literal 63 is the expected command count. The same 63 is also written in composed_commands_registry.rs at lines 21 and 160. The count is spelled out at each site, so a future command change must be edited in three places across two files. Define one shared constant for the composed command count, for example `pub const EXPECTED_COMPOSED_COMMAND_COUNT: usize = 63;` in a test-support module of swissarmyhammer-kanban. Use that constant at all three sites.
- [x] `crates/swissarmyhammer-kanban/tests/composed_commands_registry.rs:21` `code-hygiene/magic-numbers` — The literal 63 repeats the expected composed command count. It also appears at line 160 of this file and at builtin_commands.rs:190. Replace the literal with the shared named constant that holds the composed command count, and use the same constant at lines 160 and in builtin_commands.rs.
