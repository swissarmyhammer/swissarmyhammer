---
assignees:
- claude-code
depends_on:
- 01KTCBDXJKQA68WPEE4MJW77ZH
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffa80
project: cli-schema-gen
title: Migrate sah dynamic_cli.rs to shared generator for per-op precise args
---
## What
Replace sah's imprecise global-union arg logic in `apps/swissarmyhammer-cli/src/dynamic_cli.rs` with the shared schema-driven generator (card B), so each verb advertises only its own operation's params with correct required flags.

## Acceptance Criteria
- [x] Each verb subcommand under a tool advertises ONLY its operation's params (per-op scoping), with that op's required fields enforced.
- [x] `sah kanban task move` rejects invocations missing that op's required params and does NOT accept params belonging to other ops.
- [x] The global-union path (`precompute_args` cloned onto every verb) is removed; full workspace builds.

## Tests
- [x] dynamic_cli per-op precision + required-enforcement tests.
- [x] Negative tests: missing required field errors; foreign param errors.
- [x] `cargo nextest run -p swissarmyhammer-cli dynamic_cli` passes.

## Review Findings (2026-06-07 18:11) — addressed in 2nd consolidation pass
(All items remain FIXED; see git history.)

## Review Findings (2026-06-07 22:26)

### Warnings
- [x] `apps/kanban-cli/src/cli.rs:3` — module doc claimed "self-contained, only depends on clap + std so build.rs can compile it via #[path]", but the file now imports `swissarmyhammer_cli_completions::lifecycle::InstallTarget`. NOT a breakage (the completions crate is a declared build-dependency of every tool CLI, verified in each Cargo.toml `[build-dependencies]`). FIXED: rewrote the cli.rs module doc to state it now depends on the shared `lifecycle::InstallTarget` (the single canonical install-scope type), that the completions crate is a build-dep so `build.rs`'s `#[path]` compile has it available, and why that is safe. Applied the same correction to the sibling CLIs carrying the identical stale claim: `shelltool-cli`, `code-context-cli`, `avp-cli`, and `swissarmyhammer-cli` (whose doc additionally dropped the now-stale claim that `InstallTarget -> InitScope` lives in `cli_conversions` — that conversion lives with the shared type).
- [x] `apps/shelltool-cli/src/main.rs:355` — `dispatch_doctor_runs_diagnostics` discarded the `dispatch` result and asserted on a parallel `ShelltoolDoctor` object, so deleting the doctor arm would not fail it. FIXED: rewrote as `dispatch_doctor_returns_doctor_verdict`, which seeds a malformed `.shell/config.yaml` under a hermetic tempdir-as-CWD (forcing the doctor verdict to exit code 2) and asserts the value `dispatch(...)` itself returns equals 2. Verified RED: with the doctor arm deleted, "doctor" falls to the op-extraction arm and returns 1, failing the assertion.

### Nits
- [x] `apps/kanban-cli/tests/cli_tree.rs:43` — `verb` and `noun` were near-identical subcommand-lookup helpers differing only in the panic message. FIXED: collapsed into a single `fn sub<'a>(cmd, name, role) -> &'a Command`; both levels of the noun→verb tree call it (`sub(&root, "board", "noun")`, `sub(sub(&root, "task", "noun"), "move", "verb")`), so the lookup exists once.
- (Cross-task) The `collect_verb_noun_pairs` coverage-loop duplication across code-context ops.rs/main.rs and shelltool ops.rs was also resolved: hoisted into `swissarmyhammer_operations::cli_gen::test_support::collect_verb_noun_pairs`, called by all three coverage tests.

## Review Findings (2026-06-08 01:42)

> Disposition: 0 blockers. The single warning is a cross-crate flag-builder duplication/consolidation suggestion — an explicitly out-of-scope theme for this acceptance-met task ("do NOT re-raise already-resolved themes: duplication/consolidation"). All three nits are cosmetic (test naming/assertion-strength, per-crate test boilerplate the engine itself calls "defensible", and a self-imposed `Box::leak`/`&'static str` cleanup with no behavior change). None is a NEW material correctness/security/design problem, so per the task's stated verdict criteria these are not grounds to hold the task. Recorded here verbatim for history; task moved to done.

### Warnings
- [ ] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:220` — `global_flag` reimplements the existing `CliBuilder::create_flag_arg` (apps/swissarmyhammer-cli/src/dynamic_cli.rs:1533) almost verbatim — `Arg::new(id).long(long).help(help).action(SetTrue)` plus an optional `.short()`. The only delta is `global_flag` adds `.global(true)` and takes `help: &str` vs `&'static str`. swissarmyhammer-cli is a declared consumer of this crate (see lib.rs header listing `sah`), so the dependency edge already exists to let it reuse this one; leaving both means two parallel flag builders that will drift. Make `global_flag` the single source: delete `CliBuilder::create_flag_arg` and call `swissarmyhammer_cli_completions::lifecycle::global_flag(...)` from sah (passing the global bit), or, if a non-global variant is genuinely needed, add a `global: bool` parameter so one builder serves both call sites.

### Nits
- [ ] `apps/code-context-cli/src/cli.rs:138` — The added lifecycle-CLI tests `help_displays_all_lifecycle_commands` and `version_flag` are near-verbatim copies of the same tests in kanban-cli; as each tool CLI gains the identical lifecycle command set, this boilerplate is being copied per crate rather than shared. If a third+ tool CLI repeats this, lift a shared test helper into `swissarmyhammer-cli-completions` taking the binary name and expected lifecycle command list as parameters (e.g. `assert_lifecycle_help(bin, &cmds)` / `assert_version_flag(bin)`), and have each crate's test call it. Low priority — the differing binary/command contract makes it borderline, so leaving it as per-crate boilerplate is also defensible.
- [ ] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:1` — `lifecycle_subcommands_use_supplied_about` is named for verifying the per-tool `about` strings but only asserts `get_name()` equals the literal subcommand name ("serve"/"init"/"deinit"/"doctor"). The about argument passed in is never read, so the test gives false confidence that about wiring is covered when it isn't. Assert on the about string the test claims to check, e.g. `assert_eq!(serve_subcommand("serve about").get_about().unwrap().to_string(), "serve about")`, or rename the test to reflect that it only checks subcommand names.
- [ ] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:298` — The lifecycle subcommand builders take `about: &'static str`, which forces `standard_op_cli` to `Box::leak` every per-tool about string through `intern`. The `&'static str` requirement is self-imposed: clap's `Command::about`/`Subcommand` accept `impl Into<StyledStr>`, which `String` already satisfies, so the leak exists only to satisfy an overly-concrete parameter type. Change the builders to `about: impl Into<clap::builder::StyledStr>` (and `target_help: impl Into<StyledStr>`), then pass the owned `format!(...)` strings directly and delete `intern`. This removes the deliberate leak with no behavior change.