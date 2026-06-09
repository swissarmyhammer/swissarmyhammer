---
assignees:
- claude-code
depends_on:
- 01KTCBDXJKQA68WPEE4MJW77ZH
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff780
project: cli-schema-gen
title: Migrate kanban-cli onto the shared generator; delete its private cli_gen.rs
---
## What
Switch kanban-cli to the shared `swissarmyhammer_operations::cli_gen` generator (card B) and delete the now-duplicate `apps/kanban-cli/src/cli_gen.rs`.

## Acceptance Criteria
- [x] `apps/kanban-cli/src/cli_gen.rs` is deleted and `mod cli_gen;` removed from `main.rs`.
- [x] `kanban` CLI produces the identical noun/verb/arg command tree as before.
- [x] `cargo build -p kanban-cli` succeeds with no unused-dep/import warnings.

## Tests
- [x] Golden command-tree test in `apps/kanban-cli` (`tests/cli_tree.rs`).
- [x] Regression: `kanban task move` enforces that op's required field and does NOT accept the global union.
- [x] `cargo nextest run -p kanban-cli` passes.

## Review Findings (2026-06-07 18:10) — addressed in 2nd consolidation pass
(All items remain FIXED; see git history. Macro serde-default item completed in the 2nd pass.)

## Review Findings (2026-06-07 22:26)

### Warnings
- [x] `apps/code-context-cli/src/commands/ops.rs:174` — verb-noun-pair collection loop copy-pasted across three coverage tests. FIXED: added `swissarmyhammer_operations::cli_gen::test_support::collect_verb_noun_pairs(nouns)` next to the shared `parse_argv`; all three coverage tests (code-context ops.rs + main.rs, shelltool ops.rs) now call it, so the nested noun→verb loop lives in exactly one place.
- [deferred: pre-existing kanban-domain item, out of scope per task] `crates/swissarmyhammer-kanban/src/board/get.rs:37` — GetBoard::execute length.

### Nits
- [x] `apps/kanban-cli/src/main.rs:1` — `dispatch` help/error strings hardcoded `"kanban"` despite `const PROGRAM`. FIXED: both `error!` messages now interpolate `{PROGRAM}` (`Run '{PROGRAM} {name} --help' or '{PROGRAM} --help' ...` and `No command specified. Run '{PROGRAM} --help' ...`). Grep proof: the only remaining `"kanban"` literals in main.rs are the `const PROGRAM` definition and the `kanban://open/` deep-link URL scheme (a protocol id, not the program name).
- [x] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:48` — `InstallTarget` had a `DEFAULT` const but no `Default` impl. FIXED: added `impl Default for InstallTarget` delegating to `InstallTarget::DEFAULT` (single source; `Default::default()` is not const, so `DEFAULT` stays the const default the clap arg reads). Added `default_matches_default_const` test.
- [x] `crates/swissarmyhammer-operations-macros/src/lib.rs:309` — single-caller `serde_args_contain_default` wrapper. FIXED: inlined the one-line `.any()` token scan into `has_serde_default`, folding the top-level-only explanation into that function's comment. Regression tests (`list_valued_item_before_default_does_not_mask_it`, `bound_item_before_default_does_not_mask_it`, bare/valued/none/non-serde) still pass.

## Review Findings (2026-06-08 01:42) — final re-review, non-blocking

Acceptance criteria met and all prior findings resolved. The items below are either the already-resolved duplication/consolidation theme, the explicitly out-of-scope deferred `get.rs` items, or cosmetic nits (doc comments, trait derives, lifetime ergonomics). Zero blockers, zero new material correctness/security problems — task moved to DONE.

### Warnings
- [ ] `apps/code-context-cli/src/cli.rs:64` — default install scope written as literal `InstallTarget::Project` in derive attrs instead of `default_value_t = InstallTarget::DEFAULT`. (Duplication/single-source theme — already addressed for `InstallTarget`; non-blocking.)
- [ ] `apps/code-context-cli/src/cli.rs:83` — multi-shell completion `long_about` help block hand-maintained as a literal that duplicates the shared `completion_subcommand(bin_name)` template. (Duplication/consolidation theme; non-blocking.)
- [ ] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:119` — lifecycle subcommand builders take `about: &'static str`, forcing `standard_op_cli` to `Box::leak` its `format!` strings; could be `impl Into<StyledStr>`/`String`. (Clarity/ergonomics; non-blocking.)
- [ ] `crates/swissarmyhammer-kanban/src/board/get.rs:42` — GetBoard::execute ~109 lines, over the ~50-line limit. (Explicitly out-of-scope deferred kanban-domain item per task scope.)

### Nits
- [ ] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:50` — `InstallTarget::as_str` re-declares the ValueEnum token literals; guarded by `as_str_matches_value_enum_token`. (Duplication theme; already test-guarded.)
- [ ] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:90` — new public builder functions lack per-item doc comments. (Cosmetic.)
- [ ] `crates/swissarmyhammer-kanban/src/board/get.rs:28` — `GetBoard::include_counts()` single-caller wrapper. (Out-of-scope get.rs; cosmetic.)
- [ ] `crates/swissarmyhammer-operations/src/parameter.rs:16` — `ParamMeta` missing `PartialEq/Eq/Hash`. (Pre-existing; this change only touched the doc comment.)