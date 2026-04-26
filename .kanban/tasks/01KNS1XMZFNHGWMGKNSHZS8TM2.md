---
assignees:
- claude-code
depends_on:
- 01KNS1WP3ZEAKNNAD6G3WAGSEK
position_column: done
position_ordinal: ffffffffffffffffffffffffff9a80
project: code-context-cli
title: Implement doctor command for code-context CLI
---
## What
Create `code-context-cli/src/doctor.rs` with `CodeContextDoctor` implementing `DoctorRunner`, mirroring `shelltool-cli/src/doctor.rs`.

Checks to implement:
1. **Git repository** — warning if not in a git repo (uses `swissarmyhammer_common::utils::find_git_repository_root`)
2. **code-context in PATH** — warning if `code-context` binary not on PATH
3. **Code-context index** — call `swissarmyhammer_tools::mcp::tools::code_context::doctor::run_doctor(cwd)` and report project types and LSP availability as individual checks
4. **`.code-context/` directory** — check if `.code-context/` directory exists in cwd; info if present, warning if not (not initialized yet)

Structure:
```rust
pub struct CodeContextDoctor { checks: Vec<Check> }
impl DoctorRunner for CodeContextDoctor { ... }
impl CodeContextDoctor {
    pub fn new() -> Self
    pub fn run_diagnostics(&mut self) -> i32
    fn check_git_repository(&mut self)
    fn check_code_context_in_path(&mut self)
    fn check_index_directory(&mut self)
    fn check_lsp_status(&mut self)
}
pub fn run_doctor(verbose: bool) -> i32
```

The `check_lsp_status` iterates `run_doctor(cwd).lsp_servers` and creates one Check per LSP — `CheckStatus::Ok` if installed, `CheckStatus::Warning` with `install_hint` if not.

## Acceptance Criteria
- [ ] `cargo check -p code-context-cli` passes
- [ ] `run_doctor(false)` returns exit code 0, 1, or 2
- [ ] At least 4 checks are produced

## Tests
- [ ] `test_new` — empty checks
- [ ] `test_run_diagnostics` — produces ≥ 4 checks, exit code ≤ 2
- [ ] `test_check_git_repository` — produces exactly 1 check named "Git Repository"
- [ ] `test_check_code_context_in_path` — produces exactly 1 check named "code-context in PATH"
- [ ] `test_check_index_directory` — produces exactly 1 check named "Index Directory"
- [ ] Run `cargo test -p code-context-cli doctor` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.