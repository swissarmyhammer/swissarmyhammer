---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffa080
title: Add tests for shelltool-cli doctor error-path branches
---
shelltool-cli/src/doctor.rs:64-71, 105-114, 128

Coverage: 116/136 (85.3%)

Uncovered lines: 64-71, 81, 105-114, 128

**Gap 1 — `check_git_repository` not-in-git branch (lines 64-71):**
```rust
None => {
    self.add_check(Check {
        name: "Git Repository".to_string(),
        status: CheckStatus::Warning,
        message: "Not in a Git repository".to_string(),
        fix: Some("Run from within a Git repository or run `git init`".to_string()),
    });
}
```
The existing `test_check_git_repository` runs inside a git repo, so the `Some(path)` arm is covered but the `None` arm isn't. Use `tempfile::TempDir` + `env::set_current_dir(temp)` to run the check outside any git repo, then assert the check has `CheckStatus::Warning` and the "Not in a Git repository" message. Restore cwd after.

**Gap 2 — `check_shelltool_in_path` not-found branch (lines 105-114):**
```rust
} else {
    self.add_check(Check {
        name: "shelltool in PATH".to_string(),
        status: CheckStatus::Warning,
        message: "shelltool not found in PATH".to_string(),
        fix: Some("Add shelltool to your PATH or install with `cargo install --path shelltool-cli`".to_string()),
    });
}
```
The existing test runs with whatever PATH the dev/CI has — usually covers the `found` branch. Use an RAII guard to temporarily set `PATH` to an empty tempdir and assert the Warning branch fires.

**Gap 3 — `check_shell_tool_health` HealthStatus::Error mapping (line 128):**
```rust
HealthStatus::Error => CheckStatus::Error,
```
Cannot be triggered directly from the real `ShellExecuteTool` in a unit test without manipulating config state. **Low priority — skip unless you can cheaply trigger an Error status by mutating the test environment (e.g., setting an invalid shell config path).**

**Not actionable:** line 81 (`"shelltool.exe"` branch) is Windows-only and `cfg!(windows)` is false on the CI platform. Leave uncovered.

Tests go into the existing `#[cfg(test)] mod tests` block in `doctor.rs`. Use `serial_test` or a mutex to serialize tests that change `env::current_dir` or `env::set_var("PATH", ...)`. #coverage-gap