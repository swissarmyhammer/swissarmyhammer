# CLI Command Naming Clarification

## Current State (as of 2025-11-12)

### Existing Commands
- **`sah do_todos`** - EXISTS (with underscore)
  - Description: "Autonomously work through all pending todo items"
  - Shortcut for: `flow do_todos`
  - Workflow file: `builtin/workflows/do_todos.md`

- **`sah do`** - DOES NOT EXIST
  - Attempting to run shows: "error: unrecognized subcommand 'do'"

### Workflow Files
- `builtin/workflows/do_todos.md` - Current workflow file (with underscore)
- No `builtin/workflows/do.md` file exists yet

## Proposed Migration (from ideas/eliminate-issues-and-memos-migration.md)

The migration document proposes:
- **Rename workflow**: `do_todos` → `do`
- **Rename CLI command**: `sah do_todos` → `sah do`
- **Rationale**: Simplify and make it the main implementation loop

## Naming Convention Analysis

Looking at existing workflow shortcuts in the CLI:
- `do_issue` (underscore)
- `do_todos` (underscore)
- `example-actions` (hyphen)
- `greeting` (no separator)
- `hello-world` (hyphen)
- `implement` (no separator)

**Pattern**: The CLI uses the exact workflow filename (without .md) as the command name. Workflow files use either underscores or hyphens, and these are preserved in the CLI command.

## Answer to Todo Question

**Current command**: `sah do_todos` (with underscore)
**Proposed command**: `sah do` (no separator needed - simple word)
**Migration path**: Rename workflow file from `do_todos.md` to `do.md`

The CLI will automatically create the `sah do` shortcut once the workflow file is renamed.