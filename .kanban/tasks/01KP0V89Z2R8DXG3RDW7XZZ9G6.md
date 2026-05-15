---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffa180
project: kanban-mcp
title: 'shelltool-cli: add commands/skill.rs, move serve/doctor/registry into commands/'
---
## What

Adopt the `commands/` directory convention in shelltool-cli, matching sah-cli's structure.

1. Create `commands/` directory with `mod.rs`
2. Move `src/serve.rs` → `src/commands/serve.rs`
3. Move `src/doctor.rs` → `src/commands/doctor.rs`
4. Move `src/registry.rs` → `src/commands/registry.rs`
5. Create `src/commands/skill.rs` — extract skill deployment from `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` into a dedicated module matching code-context-cli's skill.rs pattern

Top-level keeps: `main.rs`, `cli.rs`, `banner.rs` (infrastructure).
`commands/` gets: `serve.rs`, `doctor.rs`, `registry.rs`, `skill.rs` (command implementations).

## Acceptance Criteria
- [x] `shelltool-cli/src/commands/` exists with serve, doctor, registry, skill
- [x] Top-level serve.rs, doctor.rs, registry.rs removed
- [x] `commands/skill.rs` has `ShelltoolSkillDeployment` implementing `Initializable`
- [x] Skill deployment removed from `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`
- [x] `shelltool serve`, `shelltool init`, `shelltool deinit`, `shelltool doctor` all still work
- [x] `cargo test -p shelltool-cli -p swissarmyhammer-tools` passes

## Review Findings (2026-04-12 10:45)

### Warnings
- [x] `shelltool-cli/src/main.rs` — Five tests deleted without replacement. The previous `main.rs` had tests for `FileWriterGuard::write`, `FileWriterGuard::flush`, `dispatch_command` Init arm, `dispatch_command` Deinit arm, and `dispatch_command` Doctor arm. These were not moved to other modules. `FileWriterGuard` still exists in main.rs but now has zero test coverage. The `dispatch_command` routing tests covered real integration paths (CWD isolation, registry invocation). Restore the `FileWriterGuard` unit tests and the `dispatch_command` routing tests, or move them to a `#[cfg(test)] mod tests` block in main.rs.
- [x] `shelltool-cli/src/main.rs` — Init and Deinit arms in `dispatch_command` duplicate the target-to-scope conversion and error-checking logic (15 nearly identical lines). The old code had `install_target_to_scope()` and `any_init_error()` helpers that both arms shared. The inlined version is harder to maintain. Extract the shared logic back into local helper functions.
- [x] `shelltool-cli/src/main.rs` — Tracing setup inlined from three small documented functions (`build_tracing_filter`, `try_init_file_tracing`, `init_stderr_tracing`) into a single 40-line block in `main()`. The closure `make_filter` partially replaces `build_tracing_filter`, but the file-logging and stderr-fallback paths lost their function boundaries and doc comments. This makes `main()` harder to read and the tracing setup harder to test in isolation. Consider restoring the helper functions.
- [x] `shelltool-cli/src/commands/skill.rs` + `code-context-cli/src/commands/skill.rs` — Approximately 170 lines of production code are duplicated verbatim: `SkillFrontmatter`, `resolve_skill`, `render_skill`, `format_skill_md`, `validate_skill_name`, `write_and_deploy`, plus their tests. This is a maintenance hazard — a bug fix in one file must be manually replicated in the other. Consider extracting these into a shared crate (e.g., `swissarmyhammer-skills` or a new `swissarmyhammer-skill-deploy` crate) or at minimum into a shared module re-exported by both CLIs.

### Nits
- [x] `shelltool-cli/src/commands/registry.rs` — The doc comment on `register_all` states `ShellExecuteTool` runs at priority 20, but `ShellExecuteTool` does not override `Initializable::priority()` so it defaults to 0. Actual execution order is: `ShellExecuteTool` (0), `ShelltoolMcpRegistration` (10), `ShelltoolSkillDeployment` (30). Update the doc comment to reflect the real priorities.

## Review Findings (2026-04-12 11:40)

### Nits
- [x] `shelltool-cli/src/commands/registry.rs` — The inner doc comment on `ShelltoolMcpRegistration::priority()` still says `Priority 10 — runs before ShellExecuteTool (priority 20).` Two problems: (1) `ShellExecuteTool` uses the trait default priority `0`, not `20`, and (2) because lower priority runs first (per `swissarmyhammer-common/src/lifecycle.rs` `sorted_indices`), `ShelltoolMcpRegistration` (10) runs **after** `ShellExecuteTool` (0), not before. The `register_all` doc comment at the top of the file was correctly updated in the prior pass, but this method-level comment was missed. Update to: `Priority 10 — runs after ShellExecuteTool (priority 0, the default).`