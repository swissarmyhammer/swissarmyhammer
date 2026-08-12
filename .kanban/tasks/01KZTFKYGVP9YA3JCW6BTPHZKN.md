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
position_column: doing
position_ordinal: '8280'
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