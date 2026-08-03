---
assignees:
- claude-code
position_column: todo
position_ordinal: e780
title: 'shell tests: stop `ShellExecuteTool::new()` in tests from making a `.shell` dir in the crate directory'
---
## What

`cargo nextest run -p swissarmyhammer-tools` creates a `.shell` directory in
`crates/swissarmyhammer-tools/`. Nextest runs each test binary with the crate
directory as the CWD, and `ShellExecuteTool::new()` calls `ShellState::new()`,
which makes `.shell` relative to the CWD.

Found while working ^mbran97. The directory is not tracked by git, but it is
litter in the source tree, and it makes test runs write outside their temp
sandbox.

Tests that call `ShellExecuteTool::new()` instead of the test-only
`ShellExecuteTool::new_isolated()`:

- `crates/swissarmyhammer-tools/src/mcp/tool_config.rs` — three tests in the
  watcher `mod tests` block.
- `crates/swissarmyhammer-tools/tests/integration/file_size_limits.rs` — the
  `register_shell_tool` helper.

`new_isolated()` is `#[cfg(test)]` and `pub(crate)`, so the integration test in
`tests/` cannot reach it. That one needs a different route — a `CurrentDirGuard`
on a temp dir, or a crate-public test constructor.

### Subtasks

- [ ] Move the `tool_config.rs` tests to `new_isolated()`.
- [ ] Give `file_size_limits.rs` an isolated state directory.
- [ ] Add a test that proves a full `-p swissarmyhammer-tools` run leaves no
      `.shell` directory in the crate directory.

## Acceptance Criteria

- [ ] `cargo nextest run -p swissarmyhammer-tools` leaves no `.shell` directory
      in `crates/swissarmyhammer-tools/`.
- [ ] No test in the crate calls `ShellExecuteTool::new()`. #bug #shelltool #test-hygiene #tools