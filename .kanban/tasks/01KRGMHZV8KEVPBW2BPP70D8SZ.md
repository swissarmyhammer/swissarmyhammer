---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffc380
project: rebuild-index
title: Rename `build status` op to `rebuild index`
---
The op named `build status` actually does an UPDATE that resets `ts_indexed`/`lsp_indexed` flags — it's a write that triggers re-indexing, not a status query. Its sibling `get status` reads, and `clear status` deletes. The name `build status` lies about both intent and side effects.

Rename to `rebuild index` (verb=`rebuild`, noun=`index`). This is purely a rename pass; behavior stays the same in this card (turning it into a synchronous rebuild is a later card).

## Scope

- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`
  - `BuildStatus` struct → `RebuildIndex` (verb/noun, description, params, lazy static, dispatch match arm, op-list strings in error messages, module doc comment)
  - `execute_build_status` → `execute_rebuild_index`
  - Update tests that assert on `"build status"` op string
- `swissarmyhammer-code-context/src/ops/status.rs`
  - `build_status()` fn → `rebuild_index()` (and re-export site in `lib.rs`)
  - `BuildStatusResult` → `RebuildIndexResult`
  - `BuildLayer` stays (it's still about which layer to reset)
  - Update doc comments and unit tests
- `code-context-cli/src/commands/ops.rs` — `"build status"` → `"rebuild index"`
- `code-context-cli/src/cli.rs` — `Commands::Build` variant → `Commands::Rebuild` (with subcommand `Index`); update help text. Drop the `build` verb entirely if it has no other subcommands.
- Update shell completions (regen via the project's completion build step, don't hand-edit)
- Update `code-context-cli/.skills/code-context/SKILL.md` and any other docs that mention `build status`

## Out of scope

- Making the rebuild synchronous (next card)
- Progress events (later card)
- Follower error message (later card)

#refactor #code-context #rebuild-index

## Review Findings (2026-05-13 13:50)

### Warnings
- [x] `builtin/skills/plan/references/PLANNING_GUIDE.md:19` — Still references `op: "build status"` as the trigger for incomplete index. This is the source-of-truth file (the `.skills/` mirror is generated from it and still shows the old name on lines 217 and 235 of `.skills/code-context/SKILL.md` and line 19 of `.skills/plan/references/PLANNING_GUIDE.md`). Update the bullet to `op: "rebuild index"` and regenerate `.skills/`.
- [x] `code-context-cli/src/cli.rs:531-534` — The `help_displays_all_top_level_commands` test still asserts the help text contains `"build"` as one of the top-level commands. After the rename the top-level command is `rebuild`, so this assertion passes only because `help.contains("build")` matches `rebuild` as a substring. The test no longer verifies what it claims. Replace `"build"` with `"rebuild"` in the command list.

### Nits
- [x] `code-context-cli/src/commands/ops.rs:305` — Doc comment on `build_simple_args` reads `(list, build, clear, lsp, detect)`. The variant is now `Rebuild`. Update to `(list, rebuild, clear, lsp, detect)`.
- [x] `ideas/code-context-architecture.md:454,636` — Design doc still uses `build status` in the section header and in the LSP restart bullet. Task scope explicitly called out updating "any other docs that mention `build status`". Rename to `rebuild index` (or note that this doc is frozen as a historical proposal).