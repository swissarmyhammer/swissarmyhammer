---
assignees:
- claude-code
depends_on:
- 01KTCBDXJKQA68WPEE4MJW77ZH
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff980
project: cli-schema-gen
title: Add schema-driven shell op commands to shelltool-cli
---
## What
Surface the shell tool's operations as CLI commands in shelltool-cli, generated from the shell tool's FULL schema via the shared generator (card B).

## Acceptance Criteria
- [x] `shelltool` exposes its shell operations as schema-generated subcommands.
- [x] Lifecycle commands (serve/init/deinit/doctor/completion) still work; build.rs generation still compiles against the static `cli.rs`.
- [x] A generated op invocation reaches `ShellTool::execute` and returns output.

## Tests
- [x] `apps/shelltool-cli/src/commands/ops.rs` integration tests.
- [x] Command-tree coverage test: generated nouns/verbs match `ShellTool::operations()` op strings.
- [x] `cargo nextest run -p shelltool-cli` passes.

## Review Findings (2026-06-07 18:10) — addressed in 2nd consolidation pass
(All items remain FIXED; see git history.)

## Review Findings (2026-06-07 22:26)

### Warnings
- [x] `apps/kanban-cli/src/main.rs:109` — program name 'kanban' hardcoded in user-facing error strings despite `const PROGRAM`. FIXED: both `error!` messages interpolate `{PROGRAM}` (line-109 pair and line-115 no-command message), so the name lives in one place. Grep proof: only the `const PROGRAM` definition and the `kanban://` URL scheme remain.
- [x] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:237` — `standard_op_cli` rustdoc documented a phantom `version` parameter that contradicts the `(name, about, schema)` signature and the inline comment below. FIXED: deleted the stale `version` sentence and replaced it with prose stating the version is sourced internally from this crate's `CARGO_PKG_VERSION` (which matches every workspace binary), matching the implementation.
- [deferred: pre-existing kanban-domain item, flagged out of scope] `crates/swissarmyhammer-kanban/src/board/get.rs:42` — GetBoard::execute length.

### Nits
- [deferred: discretionary, out of scope per task] `crates/swissarmyhammer-kanban/src/schema.rs:289` — magic `10` lower-bound in an assert; name it `MIN_KANBAN_EXAMPLES`.
- [x] `crates/swissarmyhammer-operations-macros/src/lib.rs:227` — single-caller `serde_args_contain_default` wrapper. FIXED: inlined the one-line `.any()` token scan into `has_serde_default`, moving the top-level-only explanation into that function's comment block. Regression tests still pass.

## Review Findings (2026-06-08 01:42)

### Nits
- [ ] `apps/kanban-cli/src/cli.rs:12` — The module doc comment is now inaccurate after the refactor: it says the noun/verb commands are 'built dynamically in main.rs via cli_gen' and that this file 'only defines the four lifecycle commands: serve, init, deinit, doctor'. But this crate no longer has a cli_gen module (main.rs now uses lifecycle::standard_op_cli, and noun/verb extraction lives in swissarmyhammer_operations::cli_gen), and the Commands enum actually defines FIVE lifecycle commands — Serve, Init, Deinit, Doctor, and Completion. Inaccurate docs mislead the next reader about where command-building lives. Update the doc to: noun/verb commands are built dynamically via swissarmyhammer_operations::cli_gen (assembled through lifecycle::standard_op_cli in main.rs), and this file defines the five lifecycle commands: serve, init, deinit, doctor, completion.
- [ ] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:298` — The lifecycle subcommand builders take `about: &'static str`, which forces `standard_op_cli` to permanently leak its runtime-built `about` strings through `intern`/`Box::leak` (lines 270-287). clap's `.about()` accepts `impl IntoResettable<StyledStr>`, which `String` already satisfies, so the `&'static str` bound is the only reason the leak exists — it's an avoidable concrete-type constraint per 'accept generics, not concrete types'. Change the builders to `about: impl Into<String>` (and pass the `String` straight to `.about()`), then delete `intern`/`Box::leak` and pass the `format!(...)` results directly. The leak is bounded (once per process), so this is a cleanliness/correctness nit rather than a blocker.
- [ ] `crates/swissarmyhammer-operations-macros/src/lib.rs:0` — The six `has_serde_default` tests (bare_default_is_detected, valued_default_is_detected, no_default_is_not_detected, list_valued_item_before_default_does_not_mask_it, bound_item_before_default_does_not_mask_it, non_serde_attribute_ignored) are parallel arms over a known set that differ only in (input attribute, expected bool). That is a case table, not six code paths — adding a future case means copying the boilerplate again rather than appending a row. Consider a table-driven form keeping the scenario docs as a column, e.g. `for (attr, expected, why) in [(parse_quote!(#[serde(default)]), true, "bare default"), ...] { assert_eq!(has_serde_default(&[attr]), expected, "{why}"); }`. Low priority — the per-fn names double as regression documentation, so this is a judgment call, not a defect.
- [ ] `crates/swissarmyhammer-operations/src/cli_gen.rs:639` — `parse_argv` takes `root_name: &'static str`, which is more restrictive than necessary for a public (feature-gated `test-helpers`) API — a caller holding an owned or non-'static `String` binary name cannot pass it without leaking. Accept `impl Into<clap::builder::Str>` (or document the `'static` constraint as deliberate). Minor: in practice all call sites pass string literals, so this is low-impact.

_Resolution: zero blockers, zero warnings. All acceptance criteria met and all prior actionable findings resolved. The four 2026-06-08 items are cosmetic nits (doc-comment wording, generics-vs-concrete cleanliness, table-driven test style) — per the task directive, not grounds to hold an acceptance-met task. Task moved to done._