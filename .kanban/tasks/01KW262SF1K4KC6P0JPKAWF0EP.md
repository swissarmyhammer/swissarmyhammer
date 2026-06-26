---
assignees:
- claude-code
depends_on:
- 01KW25ZW4NED0J1BD77HPK7DNX
- 01KW260M8QZ8T37A8RZGDDVZ81
position_column: todo
position_ordinal: ab80
project: expect
title: 'expect init: scaffold .expect/ tree + surface auto-detection'
---
## What
Implement `expect init` (the `Initializable` trait verb, rolling up to `sah init`) that scaffolds the `.expect/` dot-folder. Per `ideas/expect.md` §"expect init". Idempotent, never overwrites existing files.

- Implement `Initializable` for `ExpectTool` (replacing the `impl_empty_initializable!` stub), mirroring `KanbanTool`'s impl (`crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:198-363`) and `ProjectStructure` (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`).
- Scaffold at repo root:
  ```
  .expect/
    config.toml        # written with detected surface defaults
    README.md          # what expectations are + how to write one
    example.expect.md  # one worked expectation, ready to copy
    expectations/      # repo-global specs
    goldens/           # committed
    received/          # gitignored
    .gitignore         # ignores received/, keeps goldens/
  ```
- `.gitignore`: mirror `ensure_gitignore_entries` (`crates/swissarmyhammer-kanban/src/board/init.rs:313`) — explicit `received/` entry, NOT a blanket `*`, so `goldens/` stay tracked.
- Surface auto-detection: call `detect_projects()` (`crates/swissarmyhammer-project-detection`), map detected `ProjectType`s to sensible `surface` defaults written into `config.toml` so the first `expect expectation create` has context.
- Register the component in `apps/swissarmyhammer-cli/src/commands/registry.rs` `register_all` (alongside `ProjectStructure`/`KanbanTool`) so `sah init` runs it.
- Gate filesystem work to `InitScope::Project|Local` (not User), as kanban does.

## Acceptance Criteria
- [ ] `expect init` (and `sah init`) create the full `.expect/` tree; re-running does not overwrite `config.toml`/`example.expect.md`.
- [ ] `.expect/.gitignore` ignores `received/` but not `goldens/` (no blanket `*`).
- [ ] `config.toml` is written with surface defaults derived from the detected project type(s).
- [ ] Running in a non-git / read-only CWD does not panic (derive root safely; mirror gui-cwd-readonly guidance).

## Tests
- [ ] Init test in a `tempfile` repo: asserts every scaffolded path exists, `.gitignore` content (received ignored, goldens kept), and idempotency on second run.
- [ ] Test that detected `ProjectType` ⇒ expected `surface` default in `config.toml`.
- [ ] `cargo nextest run -p swissarmyhammer-tools expect_init` passes.

## Workflow
- Use `/tdd`.