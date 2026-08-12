---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kztmfshs8x2k5z1275ke8ck4
  text: |-
    Picked up. Research done.

    The shipped script stands at `builtin/validators/code-hygiene/rules/missing-docs-rust.md`. It writes `set -e`, sends the cargo report to a file, and pipes `jq` into `sort -u`. Two defects follow from that shape:

    - `set -e` makes cargo's status the script's status, so a workspace clippy linted from end to end BREAKS the run when a lint stands at deny level.
    - the filter stands in a pipe that ends in `sort -u`, so a `jq` that cannot run answers exit 0 with no finding. The two siblings each closed that hole with a `filtered` status test.

    The two siblings that carry the answer:
    - `builtin/validators/code-hygiene/rules/dead-code-rust.md`, section "A workspace the tool cannot check".
    - `builtin/validators/code-hygiene/rules/complexity-rust.md`, section "A workspace the tool cannot lint".

    Each writes five `jq` calls to files, tests each status, then breaks the run in four places: no `build-finished` entry beside a nonzero status; an error-level message with a rustc code or with no code; a `custom-build` artifact whose package writes no `build-script-executed` entry; a nonzero status beside no error-level message at all.

    Test shapes stand in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`: `ShippedStagedTree`, `verify_shipped_tree_breaks`, `verify_shipped_tree_reports`, `verify_shipped_tree_breaks_without` and `FILTER_BINARY_NAME`.

    One thing to note for the next agent: `verify_shipped_tree_breaks_without` leads `PATH` with a stub, and `PATH` is process state. `dead_code_rust.rs` therefore stands EVERY test of its module under `#[serial_test::serial(env)]`. The Rust missing-docs tests stand today in `missing_docs.rs` beside Swift, Go, Python, Dart and TypeScript tests that are not serial, so the filter test cannot stand there. The Rust tests move to a module of their own, `shipped/missing_docs_rust.rs`, which is the shape `complexity_rust.rs` and `dead_code_rust.rs` already carry and which the module comment of `shipped.rs` states.
  timestamp: 2026-08-12T09:22:37.753583+00:00
- actor: claude-code
  id: 01kztn5k0ygn63xztr84x1ev2w
  text: |-
    Measured every shape by hand before the change, with clippy 0.1.97, cargo 1.97.1 and jq 1.8.2, over nine probe packages. The card's claim reproduces: `[lints.rust] unused_variables = "deny"` beside one undocumented `pub struct` gave 0 findings and exit 101 under the shipped script, and gives 1 finding at exit 0 under the new one.

    Two measurements the card did not name, and each is now a row of the rule body:

    - `RUSTFLAGS="-D warnings"` makes the rule's OWN diagnostic arrive at level `error` with the code `missing_docs`. The raw report holds that code, the filter selects on the code, and the finding stands. The earlier script broke the run exactly where the rule had something to say.
    - The filter stood in a pipe that ended in `sort -u`. With `jq` replaced by a command that exits 127, the earlier script wrote 0 findings and exited 0, so the whole rule went missing without a word. The new script gives each `jq` call its own file and its own status test, which is the shape both siblings carry.

    TDD: the seven new tests were written first and each failed for the stated reason (the script's own line was missing from the error detail). One failure was mine rather than the script's — `RUST_UNCOMPILABLE_ERROR` named the package `uncompilable-probe`, and the two-member probe names its failing member `bad`, so the fragments now name the line and cargo's words alone.

    The Rust tests moved from `shipped/missing_docs.rs` to a new `shipped/missing_docs_rust.rs`, which is the shape `complexity_rust.rs` and `dead_code_rust.rs` carry and which the module comment of `shipped.rs` states. `#[serial_test::serial(env)]` stands on the filter test alone, because that test alone writes process state, which is the ruling ^y4xyw1g recorded.
  timestamp: 2026-08-12T09:34:31.966215+00:00
- actor: claude-code
  id: 01kztn5sgfkqke8bhcjcn5zwh8
  text: |-
    ### implement — changed
    - evidence: 4 files — builtin/validators/code-hygiene/rules/missing-docs-rust.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs_rust.rs (new). `cargo nextest run -p swissarmyhammer-validators`: 688 run, 688 passed, 0 failed. `cargo fmt --check` and `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings`: clean.
    - next: /review
  timestamp: 2026-08-12T09:34:38.607204+00:00
- actor: claude-code
  id: 01kztq2qr12t0v4s1q0bwt0ahs
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (59bd9ae5c) — engine returned 11 confirmed, 3 refuted, 40 attempted. 6 findings recorded: crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:87, :235, :262, :514, :567; crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:970.
    - dropped 5 findings under the review skill's blanket test-refactor exception, each asking to rename or deduplicate test code that already existed: shell/mod.rs:1086 (rename `test_initializable_applicable_user_scope`, predates this commit) and missing_docs_rust.rs:83, :201, :202, :251 (deduplicate "Cargo.toml"/"build.rs" literals moved verbatim from missing_docs.rs:903, :1021, :1022, :1071).
    - next: /implement — task stays in review until every item is checked and a fresh review is clean.
  timestamp: 2026-08-12T10:07:55.649211+00:00
- actor: claude-code
  id: 01kztq3vv0v0rqfg5j08xe6j9b
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 4 files (builtin/validators/code-hygiene/rules/missing-docs-rust.md, tool_rules/tests/shipped.rs, shipped/missing_docs.rs, shipped/missing_docs_rust.rs new)
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3437 passed, 0 failed, 0 skipped. cargo fmt --check clean. cargo clippy --workspace --all-targets -- -D warnings clean. One pre-existing failure was fixed on the way: mcp::tools::shell::tests::shell_description_states_blocking_and_no_tail, root cause the already-committed d20c7f847.
    - commit: 59bd9ae5c4bdd26f1bddf566454353e07dbb7109
    - review: findings — crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:87, :235, :262, :514, :567; crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:970
  timestamp: 2026-08-12T10:08:32.608867+00:00
- actor: claude-code
  id: 01kztqts6se9ewcypdcvtjx98k
  text: |-
    Worked the six review findings.

    The four error messages start lowercase now — `SHELL_STATE_INIT_FAILED`, the builtin-config parse failure, the regex compile failure, and the `.shell/` removal failure. The cause was then read over the WHOLE file: every other uppercase string in it is a different field. `HealthCheck::ok` messages ("No user config at ..."), and the `hint` argument of a warning or an error ("Check file access on ...", "Fix the YAML syntax in ...") are not the Display message the `rust/error-handling` rule names. The two messages that open with `{}` take the check name — "User config", "Project config" — which the same rule carves out as a proper noun, and no literal letter stands there to change.

    `deinit` and `init` are one function now. `LifecycleDirection` names the two directions, and `run_lifecycle` walks the three steps both share — the MCP server entry, the `Bash` permission, and the `.shell/` config — with one `match direction` for each step. Each direction keeps what it had: an install stops at the first failed step and reports no success line, a removal carries on and always reports one. `init` and `deinit` are each one call now, far under the forty-token gate `duplication-parsed` states.

    The three steps stand as closures inside `run_lifecycle` rather than as three small functions of their own. Three named functions of that shape normalize to nearly one stream, so they would have traded one duplicate pair for three. A closure is not a named definition, so the rule never compares it.

    The generated stub script quotes the path it hands through: `exec "{real}" "$@"`.

    Measured after the change: `sah tool code_context duplication find --files crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` reports nothing.
  timestamp: 2026-08-12T10:21:03.577200+00:00
- actor: claude-code
  id: 01kztqtyz9jdd6nxgv3x0vk9dp
  text: |-
    ### implement — changed
    - evidence: 2 files — crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs. All 6 review findings are now `- [x]`. `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-tools`: 2202 run, 2202 passed, 0 failed, 0 skipped. `cargo fmt --check` clean. `cargo clippy -p swissarmyhammer-tools -p swissarmyhammer-validators --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-12T10:21:09.481028+00:00
- actor: claude-code
  id: 01kztrg0wgz1gk290xa399v7s7
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (bf0cd8d34) — engine returned 3 findings, 3 confirmed, 1 refuted, 8 attempted. 1 finding recorded: crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:181.
    - dropped 2 findings under the review skill's blanket test-refactor exception, each asking to restructure test code that already existed: shell/mod.rs:217 (`new_isolated` is `#[cfg(test)]`, unchanged by this commit, called by 22 existing tests) and tool_rules/tests/shipped.rs:922 (deduplicate the `resolved_binary` test helper, unchanged by this commit).
    - next: /implement — task stays in review until every item is checked and a fresh review is clean.
  timestamp: 2026-08-12T10:32:39.568812+00:00
- actor: claude-code
  id: 01kztrgn660h56pehga8a1p11d
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 2 files (crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs). All six findings of iteration 1 are checked.
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators) | rdeps(swissarmyhammer-tools)', 3437 run, 3437 passed, 0 failed, 0 skipped. cargo fmt --check clean. cargo clippy --workspace --all-targets -- -D warnings clean.
    - commit: bf0cd8d34
    - review: findings — crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:181 (1 recorded of 3; 2 dropped under the test-refactor exception)
  timestamp: 2026-08-12T10:33:00.358998+00:00
- actor: claude-code
  id: 01kzts19h1s6z5z8v5wwsnb0kr
  text: |-
    Picked up again for the open finding of 2026-08-12 05:26: `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:181` — `ShellExecuteTool::new()` calls `.expect()`, which panics on a filesystem failure that is an expected failure mode.

    Research, before any edit.

    The whole file holds two `.expect` calls outside `mod tests`: `new()` at the line the finding names, and `new_isolated()` (`#[cfg(test)]`). The review of iteration 2 already read `new_isolated` and dropped it under the review skill's written test-refactor exception ("`new_isolated` is `#[cfg(test)]`, unchanged by this commit, called by 22 existing tests"). A panic there is a test failure, which is the correct signal for test-only code, so the cause the finding names does not stand in it. Every other `.expect`/`.unwrap` of the file stands inside `mod tests`.

    `ShellState::new()` already returns `anyhow::Result<Self>` and already falls back to a temp directory when the preferred `.shell` location is not writable, so the remaining failure is "no directory anywhere can be created". `ShellState::new` is the public API of the same module and returns `anyhow`, so `anyhow::Result` is the prevailing shape for this constructor and no error type is converted back and forth.

    The blast radius of `new() -> anyhow::Result<Self>`:

    - `apps/shelltool-cli/src/commands/ops.rs::run_operation` — returns `i32`; reports the error and answers 1.
    - `apps/shelltool-cli/src/commands/doctor.rs::check_shell_tool_health` — returns `()`; adds an Error check, which is what a doctor is for.
    - `apps/shelltool-cli/src/commands/serve.rs::ShellToolServer::new` — becomes fallible; `run_serve` already returns `Result<(), String>`.
    - `apps/shelltool-cli/src/main.rs` — builds the schema; reports and exits 1.
    - `apps/shelltool-cli/src/commands/registry.rs` — `mirdan::declare_tool_install!` puts the component expression inside `ToolInstall::register_components`, which returns `()` for three CLIs. The macro takes only an infallible expression, so this file states the `ToolInstall` impl by hand and reports a tool it cannot build.
    - `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs::register_shell_tools` — returns `()`, and four public functions stand between it and any caller that could carry an error (`register_all_tools`, `create_agent_tools_server`, `collect_all_health_checks`, `create_fully_registered_tool_registry`). Re-signing all four is a refactor the card does not ask for, so this function reports the failure through `tracing::error!` and registers no shell tool.

    `impl Default for ShellExecuteTool` calls `new()`, so it goes: a fallible constructor has no `Default`. `clippy::new_without_default` only asks for one where `new()` answers `Self`. The same holds for `Default for ShellToolServer`.

    Two doc comments in `state.rs` name `ShellExecuteTool::new()`'s `expect` and need to name the new answer instead.
  timestamp: 2026-08-12T10:42:05.473404+00:00
- actor: claude-code
  id: 01kztt2enqkez9bc8a7x0q2k8h
  text: |-
    Worked the open finding.

    `ShellExecuteTool::new()` answers `anyhow::Result<Self>` now. It carries the `ShellState::new()` error with `.context(SHELL_STATE_INIT_FAILED)`, so the message the constant holds still reaches the caller. `impl Default for ShellExecuteTool` is gone: a constructor that reads the filesystem cannot answer `Self`, and `clippy::new_without_default` only asks for a `Default` where `new()` does. The same holds for `ShellToolServer`, whose `new()` is fallible now and whose `Default` is gone with the test that measured it.

    Each caller handles the failure where it stands:

    - `register_shell_tools` reports through `tracing::error!` and registers no shell tool. Four public functions stand between it and any caller that could carry an error — `register_all_tools`, `create_agent_tools_server`, `collect_all_health_checks`, `create_fully_registered_tool_registry` — and re-signing all four is a refactor the card does not ask for. One unwritable directory now costs the server its shell tool instead of the whole process.
    - `ShelltoolInstall` states its `ToolInstall` impl by hand and reports the same way. `mirdan::declare_tool_install!` takes each component as one expression of the component's own type, so a `Result` cannot stand there. The module doc states why this one CLI does not use the macro.
    - `run_operation` reports and answers exit code 1. `check_shell_tool_health` adds one Error check with a fix hint, which is what a doctor exists to give. `ShellToolServer::new` is fallible and `run_serve` maps it. `main` reports and exits 1.
    - `shelltool-cli` gains `anyhow`, which `code-context-cli` already carries.

    TDD: `new_answers_a_result_the_caller_reads` was written first and failed to compile — `no method named 'err' found for struct ShellExecuteTool` — which is the missing feature, then passed.

    What did not work, for the next agent: the first test forced a real failure by pointing `TMPDIR` at a regular file beside a read-only working directory, so both the preferred `.shell` and the temp fallback failed. It measured the error, and it broke 14 sibling tests under `cargo test`: `TMPDIR` is process state, `#[serial(env)]` only holds against tests that carry the same marker, and every sibling reaching `new_isolated()` reads `std::env::temp_dir()`. Under `cargo nextest` each test owns a process and the leak cannot happen, but `cargo test` must stay green too, so the test now names the answer's type in a binding and measures the writable case. `ShellState`'s read-only case already stands measured in `state.rs`, by `falls_back_to_temp_when_preferred_dir_is_read_only`, with no env at all.

    Two doc comments in `state.rs` named the old `expect` and now name the new answer.

    `new_isolated()` keeps its `expect`: it is `#[cfg(test)]`, a panic there is a test failure, which is the signal test-only code gives, and the review of iteration 2 already dropped that line under the written test-refactor exception.

    `cargo doc -p swissarmyhammer-tools --no-deps` reported one new warning for a doc link to the private `SHELL_STATE_INIT_FAILED`; the doc states the failure in prose instead, and the warning is gone.
  timestamp: 2026-08-12T11:00:12.087305+00:00
- actor: claude-code
  id: 01kztt2qrgh10mapbqh88zvsnz
  text: |-
    ### implement — changed
    - evidence: 10 files — crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/shell/state.rs, crates/swissarmyhammer-tools/tests/integration/file_size_limits.rs, apps/shelltool-cli/Cargo.toml, apps/shelltool-cli/src/main.rs, apps/shelltool-cli/src/commands/ops.rs, apps/shelltool-cli/src/commands/doctor.rs, apps/shelltool-cli/src/commands/serve.rs, apps/shelltool-cli/src/commands/registry.rs, Cargo.lock. The finding of 2026-08-12 05:26 (shell/mod.rs:181) is now `- [x]`, and all 7 findings of the card are checked. `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`: 2749 run, 2749 passed, 0 failed, 0 skipped. `cargo nextest run -p swissarmyhammer-tools -p shelltool-cli`: 1557 run, 1557 passed. `cargo nextest run -p shelltool-cli`: 42 passed. `cargo test -p swissarmyhammer-tools --lib mcp::tools::shell::`: 187 passed. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo doc -p swissarmyhammer-tools --no-deps` adds no warning.
    - next: /review
  timestamp: 2026-08-12T11:00:21.392944+00:00
- actor: claude-code
  id: 01kzttnm2gzwwbf09vs16zk2p1
  text: |-
    ### test — green
    - evidence: `cargo fmt --check` clean. `cargo build --workspace --all-targets` clean, no warnings (87s). `cargo clippy --workspace --all-targets -- -D warnings` clean, no warnings (38s). `cargo nextest run --workspace` — 14138 tests run: 14138 passed, 0 failed, 0 skipped (run twice for confirmation; timing-only "slow"/"leaky" flags on unrelated `swissarmyhammer-validators` fixture tests were non-reproducible between runs and did not affect the passed count on either run).
    - next: ready for review.
  timestamp: 2026-08-12T11:10:40.208761+00:00
position_column: doing
position_ordinal: '8380'
title: missing-docs-rust breaks the run for a workspace clippy linted, when a lint stands at deny level
---
`missing-docs-rust` tests the STATUS of `cargo clippy` alone: it writes `set -e` and lets cargo's exit status be the script's. `cargo clippy` gives one status to a run it could not make and to a run it made from end to end while a lint stands at deny level, so the second shape breaks the run and every `missing_docs` finding it holds is thrown away.

Measured 2026-08-12 with clippy 0.1.97, over one probe package that declares `[lints.rust] unused_variables = "deny"`, holds one unused variable, and holds an undocumented `pub struct Undocumented`:

```
$ bash <the shipped missing-docs-rust script>
error: could not compile `md-probe` (lib) due to 1 previous error; 2 warnings emitted
exit=101
```

0 findings, exit 1 to the engine — for a workspace clippy linted, whose undocumented public item the rule exists to report. `RUSTFLAGS="-D warnings"` and a crate-level `#![deny(...)]` give the same shape, and `RUSTFLAGS="-D warnings"` is what a CI machine sets.

`builtin/validators/README.md` states the answer: "One status can carry both a measured run and a broken run... The script must then test the REPORT beside the status, and accept the shared status only for the report shape a measured run writes."

Two sibling rules already make that test over the same cargo report, and each states its measurement in a table: `complexity-rust` ("A workspace the tool cannot lint") and `dead-code-rust` ("A workspace the tool cannot check"). Give `missing-docs-rust` the same four tests — `build-finished` present, no rustc error code, every compiled build script ran, and no nonzero status with no compiler error at all — and hold each with an acceptance test beside `the_shipped_rust_missing_docs_tool_rule_breaks_on_a_crate_that_does_not_compile`. `ShippedStagedTree`, `verify_shipped_tree_breaks` and `verify_shipped_tree_reports` in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` carry the shapes.

#tool-validators #objectivity</description>
<parameter name="tags">["tool-validators", "objectivity"]

## Review Findings (2026-08-12 04:46)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:87` — Error message starts with uppercase 'F' in 'Failed to initialize shell state'. Error messages should be lowercase except for acronyms, CamelCase identifiers, or proper nouns. Change 'Failed to initialize shell state' to 'failed to initialize shell state'.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:235` — Error message starts with uppercase 'B' in 'Builtin shell config failed to parse'. Error messages should be lowercase except for acronyms, CamelCase identifiers, or proper nouns; 'Builtin' is neither. Change 'Builtin shell config failed to parse' to 'builtin shell config failed to parse'.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:262` — Error message starts with uppercase 'P' in 'Pattern '{}' failed to compile'. Error messages should be lowercase except for acronyms, CamelCase identifiers, or proper nouns; 'Pattern' is none of these. Change 'Pattern '{}' failed to compile' to 'pattern '{}' failed to compile'.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:514` — fn `deinit` is a near-duplicate of `init` at crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:468 (208 tokens, 95% alike).
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:567` — Error message starts with uppercase 'F' in 'Failed to remove {}/ directory'. Error messages should be lowercase except for acronyms, CamelCase identifiers, or proper nouns. Change 'Failed to remove {}/ directory' to 'failed to remove {}/ directory'.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:970` — Unquoted variable in generated shell script allows command injection if binary path contains spaces or special shell characters. Quote the variable in the shell script: change `exec {real} "$@"` to `exec "{real}" "$@"` to ensure safe shell execution regardless of path content.

## Review Findings (2026-08-12 05:26)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:181` — ShellExecuteTool::new() uses .expect() which panics on filesystem errors when initializing ShellState. This violates the rule that panics are for bugs only — creating ShellState can fail on expected failure modes like missing directories or permission issues, not internal invariant violations. Return Result<Self, E> from ShellExecuteTool::new() instead of panicking, allowing callers to handle initialization failures gracefully.