---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffad80
project: kanban-mcp
title: 'shelltool-cli: add tests/ directory with integration tests'
---
## What

shelltool-cli has no `tests/` directory — the other CLIs (code-context-cli, swissarmyhammer-cli, kanban-cli) all have integration tests. Add `shelltool-cli/tests/cli.rs` with at minimum:

- `shelltool --help` lists all subcommands
- `shelltool doctor` exits 0, 1, or 2
- `shelltool doctor --verbose` is accepted

Model on the integration tests in the other CLIs.

## Acceptance Criteria
- [x] `shelltool-cli/tests/cli.rs` exists
- [x] At least 3 integration tests covering help, doctor, doctor --verbose
- [x] `cargo test -p shelltool-cli` passes including the new tests
