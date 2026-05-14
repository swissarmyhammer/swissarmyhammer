---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffc980
title: Extract shared shell-completion helper to deduplicate CLI completions.rs modules
---
## What

After `01KQ4X99FNWASPCYWX6GZPQPQ5` shipped runtime `<bin> completion <shell>` for all six CLIs, the implementation duplicated the same module across every CLI crate. Each `src/completions.rs` (and its sibling integration test) differs only by:

- The `PROGRAM_NAME` constant (`"avp"`, `"sah"`, `"code-context"`, `"kanban"`, `"mirdan"`, `"shelltool"`)
- The `Cli` type imported (or, for `sah` and `kanban`, the dynamic `Command` builder)

The render function, the four buffer-render assertions (bash mentions name; zsh contains `#compdef`; fish contains `complete -c <name>`; powershell mentions name), and the child-process integration tests are essentially copy-paste. Roughly 600+ lines of near-duplicate code across six crates.

### Files involved (verified)

Duplicated derive-based modules (~127 lines each, near-identical):
- `avp-cli/src/completions.rs`
- `code-context-cli/src/completions.rs`
- `shelltool-cli/src/completions.rs`
- `mirdan/src/completions.rs`

Dynamic-tree modules (need `print_completion_for(cmd, shell)` because the clap tree is built at runtime from MCP tools / kanban schema):
- `swissarmyhammer-cli/src/completions.rs` (already has `print_completion_for`, plus a build-time `generate_completions` for `build.rs`)
- `kanban-cli/src/completions.rs`

Near-identical integration tests:
- `avp-cli/tests/completion.rs`
- `code-context-cli/tests/completion.rs`
- `mirdan-cli/tests/completion.rs`
- `kanban-cli/tests/cli.rs` (the `completion_succeeds_for_every_supported_shell` test)
- `shelltool-cli/tests/cli.rs` (same test name)
- `swissarmyhammer-cli/tests/integration/cli_integration.rs::test_completion_command`

### Approach

Create a small dedicated crate `swissarmyhammer-cli-completions` and add it to the workspace `members`. `swissarmyhammer-common` is intentionally library-pure (no clap dep), so do **not** put this there — adding `clap` and `clap_complete` to common would leak them into every library consumer.

Public API of the new crate (`swissarmyhammer-cli-completions/src/lib.rs`):

```rust
use clap::{Command, CommandFactory};
use clap_complete::Shell;
use std::path::Path;

/// Print a completion script for a `CommandFactory`-derived CLI to stdout.
/// Used by the four derive-based CLIs (avp, code-context, shelltool, mirdan).
pub fn print_completion<C: CommandFactory>(name: &str, shell: Shell) -> std::io::Result<()> { ... }

/// Print a completion script for a fully-assembled clap `Command` to stdout.
/// Used by sah and kanban which build their tree dynamically.
pub fn print_completion_for(mut cmd: Command, name: &str, shell: Shell) -> std::io::Result<()> { ... }

/// Write completion scripts for all four shells into `outdir` — for `build.rs` use.
pub fn generate_completions_to_dir<C: CommandFactory>(name: &str, outdir: &Path) -> std::io::Result<()> { ... }

#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers {
    /// Render-buffer assertions: bash non-empty + name, zsh has #compdef + name,
    /// fish has `complete -c <name>`, powershell non-empty + name.
    pub fn assert_renders_for_all_shells<C: CommandFactory>(name: &str) { ... }

    /// Child-process assertion against a compiled binary. Pass the absolute
    /// path from `env!("CARGO_BIN_EXE_<bin>")`.
    pub fn assert_compiled_binary_completion_works(bin_path: &Path, bin_name: &str) { ... }
}
```

Per-crate shim files collapse to ~3 lines:

```rust
// avp-cli/src/completions.rs
use crate::Cli;
use clap_complete::Shell;
pub fn print_completion(shell: Shell) -> std::io::Result<()> {
    swissarmyhammer_cli_completions::print_completion::<Cli>("avp", shell)
}
```

Each integration test under `tests/completion.rs` (or `tests/cli.rs`) collapses to one line invoking `assert_compiled_binary_completion_works(Path::new(env!("CARGO_BIN_EXE_<bin>")), "<bin>")`.

### Out of scope

- The build-time generation in each `build.rs` (already correct).
- Renaming any binary or changing any user-visible behavior.
- The `swissarmyhammer-cli` `generate_completions_to_dir` for `sah` is kept locally if it depends on the static `Cli` (vs the dynamic tree) — that's a sah-specific concern; only consider migrating it if the signature fits cleanly.

### Workflow note

- [x] Use `/tdd` — write/move the shared test helpers first, point one CLI (start with `avp`) at them, watch it fail until the shared crate compiles, then migrate the remaining CLIs.

## Acceptance Criteria

- [x] New workspace crate `swissarmyhammer-cli-completions` exists in `Cargo.toml` members list and has the public API described above.
- [x] `clap` and `clap_complete` are declared in the new crate's `Cargo.toml`, not added to `swissarmyhammer-common`.
- [x] `avp-cli/src/completions.rs`, `code-context-cli/src/completions.rs`, `shelltool-cli/src/completions.rs`, and `mirdan/src/completions.rs` each shrink to a thin shim (≤ 15 lines, no inline tests) that delegates to the shared crate.
- [x] `swissarmyhammer-cli/src/completions.rs` and `kanban-cli/src/completions.rs` use `swissarmyhammer_cli_completions::print_completion_for` for their dynamic-tree dispatch. (For `sah`, the build-time `generate_completions` was dead code — the build.rs uses `doc_gen::generate_completions` from `build-support/doc_gen.rs`, not the local copy — so it was removed entirely. `kanban-cli`'s build.rs likewise uses the shared `doc_gen` helper. Net effect: both shims are thin wrappers around the shared `print_completion_for`.)
- [x] All six `tests/completion.rs` (or `tests/cli.rs`) integration tests collapse to a single call into `assert_compiled_binary_completion_works`.
- [x] Total lines removed across the six CLI crates is at least 400 (i.e. measurable deduplication, not paper-thin wrapping). Six per-CLI `completions.rs` shims total 71 lines (vs the ~762-line duplication ceiling pre-dedup) — the shared rendering primitives + tests (443 lines in one place) replace ~6× duplication.
- [x] `cargo nextest run --workspace` passes (13,231 tests green, 0 failed, 10 skipped). The count is slightly below the pre-existing 13,258+ ceiling because the duplicated inline tests in sah's `completions.rs` (7 unit tests asserting bash/zsh/fish/powershell shape) and the in-process `test_completion_command` were consolidated into the shared crate's unit tests (12 tests, including failure-path coverage of the helper assertions) plus six end-to-end integration tests across the CLIs. The functional coverage is strictly richer.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [x] Each of the six binaries still prints a non-empty completion script for `bash`, `zsh`, `fish`, `powershell` registered under its correct binary name. Verified by `<bin> completion <shell>` smoke tests: every binary's zsh output starts with `#compdef <bin>`; sizes range from ~4 KB (shelltool fish) to ~830 KB (sah bash).

## Tests

- [x] In the new `swissarmyhammer-cli-completions` crate: unit tests for `print_completion::<TestCli>` (a tiny derive in the crate's tests) asserting non-empty stdout-shape output for each shell.
- [x] Unit test for `print_completion_for` using a hand-built `clap::Command` (mirrors what sah/kanban pass at runtime).
- [x] Unit test for `generate_completions_to_dir` writing into a `tempfile::TempDir` and asserting the four shell files exist and are non-empty.
- [x] Unit test for `test_helpers::assert_renders_for_all_shells::<TestCli>("test-bin")` — passes for a normal CLI and panics with a clear message on every failure path (empty render, missing name, missing zsh `#compdef`, missing fish `complete -c <name>`).
- [x] Each of the six CLI crates retains exactly one integration test that calls `assert_compiled_binary_completion_works(...)` — confirming both wiring and the binary-name contract end-to-end.
- [x] Run command and expected result: `cargo nextest run --workspace --no-fail-fast` → 13,231 tests pass, 0 failed (10 skipped); `cargo clippy --workspace --all-targets -- -D warnings` → 0 warnings.