---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffb80
project: cli-schema-gen
title: 'Audit operation params: non-Option fields with serde defaults are marked CLI-required'
---
## What
The `#[operation]` macro (`crates/swissarmyhammer-operations-macros/src/lib.rs:137`) derived a parameter's `required` flag purely from whether its Rust type is `Option<T>` — `let required = !is_option_type(&field.ty);`. It ignored `#[serde(default)]`.

This means any operation field that is a non-`Option` type BUT carries a serde default (so it is genuinely optional at dispatch) got marked `required` in `x-operation-schemas`. Once a CLI honors per-op required flags (which `swissarmyhammer_operations::cli_gen` now does for kanban-cli AND sah after card 01KTCBG5GP4FS50ZFPKSSN2H6Q), those ops reject valid no-arg invocations.

This surfaced on `kanban board get`: `GetBoard.include_counts: bool` had `#[serde(default)]` + a `Default` and the dispatch read it via `op.get_bool("include_counts").unwrap_or(true)`, yet the schema marked it required, so `sah tool kanban board get` (no args) errored. Fixed in that card by modeling it as `Option<bool>`.

## Resolution
Fixed the root cause systemically in the macro rather than point-locally. A field is now `required` iff it is non-`Option` AND carries no `#[serde(default ...)]`. New `has_serde_default()` helper reads the existing serde attribute (data-driven — required-ness no longer silently coupled to `Option<>` only, and no redundant new `#[param(optional)]` annotation needed).

### Audit (all `#[operation]` structs, non-Option + serde default) — 6 fields, all in kanban
- `AddEntity.overrides: HashMap<String, Value>`
- `AddTask.assignees: Vec<ActorId>`
- `AddTask.depends_on: Vec<TaskId>`
- `AddActor.ensure: bool`
- `AddPerspective.fields: Vec<PerspectiveFieldEntry>`
- `AddPerspective.sort: Vec<SortEntry>`

No other operation crate (skills, agents, code_context, web, questions, ralph, shell, git, files) has a non-Option serde-defaulted operation field. All 6 are now correctly NOT required, with no source-level changes needed in any op struct.

## Acceptance Criteria
- [x] Enumerate every `#[operation]` struct field that is non-`Option` yet has a serde default.
- [x] For each, decide: model as `Option<T>` OR teach the macro/`ParamMeta` to treat a serde-defaulted field as not-required. (Chose: teach the macro — single systemic fix covers all 6.)
- [x] Add a regression test at the schema layer asserting a defaulted field is NOT in an op's `required` list.

## Tests added
- `crates/swissarmyhammer-operations/tests/macro_expansion.rs::test_serde_default_field_not_required` — macro/ParamMeta layer (bool, Vec, named-fn default, multi-attr serde).
- `crates/swissarmyhammer-kanban/src/schema.rs::serde_defaulted_fields_excluded_from_required_signatures` — schema layer, real macro-generated ops (`add task`, `add actor`), asserts serde-defaulted fields absent from `x-op-signatures`.

## Notes
- Considered an explicit `#[param(optional)]` / `#[param(default = ...)]` attribute but rejected it: reading the existing `#[serde(default)]` is data-driven and avoids a redundant second annotation.

## Review Findings (2026-06-08 04:29)

### Warnings
- [ ] `crates/swissarmyhammer-kanban/src/board/get.rs:37` — GetBoard::execute is ~111 lines of actual code (lines 37-164), far over the ~50-line budget. It interleaves entity reads, the no-counts early-return branch, task enrichment, a per-column counting loop, column-count JSON mapping, and summary math in one body, which makes the counting logic hard to test in isolation and hard to read. Extract the count-bearing path into helpers, e.g. a `count_columns(&[Entity]) -> (column_counts, ready_counts, total_ready)` and a `build_summary(...)` that returns the summary JSON, leaving `execute` to orchestrate read → branch → assemble. This keeps each piece under the budget and lets the counting loop be unit-tested without constructing the full board response.

### Nits
- [ ] `apps/shelltool-cli/src/commands/ops.rs:39` — `run_operation` returns a bare `i32` to encode a process exit status, where `0` and `1` are magic primitives whose meaning lives only in the doc comment; `std::process::ExitCode` is the type-safe, self-documenting return for this. Return `std::process::ExitCode` (`ExitCode::SUCCESS` / `ExitCode::FAILURE`) so the exit semantics are encoded in the type rather than in prose and stray integer values can't be returned.
- [ ] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:119` — The lifecycle subcommand builders (`serve_subcommand`, `init_subcommand`, `deinit_subcommand`, `doctor_subcommand`) take `about: &'static str`, which is stricter than clap needs (clap's `.about()` accepts `impl Into<StyledStr>`). This concrete-type constraint is precisely what forces `standard_op_cli` to `Box::leak` its runtime-derived strings via `intern`. Accept `about: impl Into<StyledStr>` (or owned `String`) so callers pass `format!(...)` results directly with no leak; the `intern` helper then disappears. Bounded startup leak, so nit, not blocker.

_Review verdict: acceptance criteria met, 0 blockers. The 1 warning and 2 nits are altitude/style observations in adjacent files (board/get.rs from the prior card, shelltool-cli, cli-completions), not material problems with this task's macro fix. Moved to done._