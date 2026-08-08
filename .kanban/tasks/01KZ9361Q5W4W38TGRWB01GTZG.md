---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzdqfddk1kj3by4kgk30779s
  text: |-
    Priority direction from the watching session (user call, 2026-08-07):

    This card is now at the top of todo. Pick it up NEXT, after the card currently in doing (^s0xv14n) closes.

    Reason: the probe chain keeps spawning follow-up detection cards (^n3exwfs, ^s0xv14n) and the picker keeps taking them, while this card — the first real tool rules through the entire machine, and the reason the machine exists — has been ready and unblocked since the schema landed. Every engine piece it needs is done: tool block (^q4909tf), execution path (^cbnfe97), doctor fixtures (^2hk89aj), install lifecycle (^mhcn5hb), project-type matching (^ygt2rre, ^3hwy2pd). Remaining probe follow-ups (^58n25xs, ^w0efc04, and any new detection gaps) queue BEHIND this card and ^f0wna3d unless they block it — file them, do not pick them first.
  timestamp: 2026-08-07T09:04:43.443247+00:00
- actor: claude-code
  id: 01kze2w3kv4vt9tkssdqs85r45
  text: |-
    Research + discoveries (read this before touching tool rules again):

    1. **Both pipelines were run in a terminal first, and the rule files hold exactly what ran.**
       - Rust, at the repo root: `cargo clippy --message-format=json --quiet -- -W missing_docs | jq -c 'select(.reason=="compiler-message") | .message | select(.code.code=="missing_docs") | select(.spans|length>0) | {file: .spans[0].file_name, line: .spans[0].line_start, message: .message}'` → 1647 findings, workspace-root-relative paths. Cold 41s, warm-cached 1.7s. The `select(.spans|length>0)` guard is there because `.spans[0].file_name` on a span-less diagnostic yields a null `file`, which breaks the stdout contract.
       - Python, in the fixtures dir: `ruff check --isolated --no-cache --select D1 --output-format json "$@" | jq -c '.[] | {file: .filename, line: .location.row, message: "\(.code) \(.message)"}'` → fail fixture 1 finding, pass fixture 0. ruff reports an ABSOLUTE `.filename`; the engine's `normalize_tool_path` strips the repo root.
       - `--isolated` replaces the README's "write a temp config and pass it with a flag". It is the stronger form of the same rule: ruff reads NO configuration file, so the script can never inherit the project's own lint config.

    2. **ENGINE GAP FOUND AND FIXED: a `workspace`-scope tool rule could never pass its fixtures.** `doctor::run_fixture` runs the script twice with the fixtures dir as cwd, and a `workspace`-scope script gets no arguments. Both runs therefore see BOTH fixture files and report the same findings — the fail fixture passes and the pass fixture always fails. No test ever covered a workspace-scope fixture (only `files` scope existed in the suite). Fixed by counting only the findings a run reports ABOUT the fixture under test, reusing `tool_rules::normalize_tool_path` so doctor and the engine attribute a path the same way. RED was observed first (`the pass fixture workspace-check.pass.txt produced 1 finding(s); none are allowed`).

    3. **Cargo lints a package, never a loose file.** The fixtures dir now carries `Cargo.toml` (with `[workspace]`, so cargo stops there and never joins the repo workspace) plus `lib.rs`, which holds both Rust fixtures as `#[path]` modules. Without it `cargo clippy` in the fixtures dir walks up to the repo root and lints all 40 crates. `target/` lands there and is gitignored; the build script and the embed guard both skip it.

    4. **ENGINE GAP FOUND AND FIXED: `sah init` never wrote the fixtures.** mirdan's build script embedded only `*.md` under `builtin/validators/`, so an installed sah would deploy the two tool rules WITHOUT their fixtures, doctor would report `MissingFixtures`, and both rules would silently fall back to the prompt rule. Added `BuiltinGenerator::all_extensions()` and a guard test that walks `builtin/validators/` and fails when any file is not embedded. RED was observed (7 fixture files reported missing).

    5. **The Rust rule declares NO install commands, on purpose.** `install_command_pins_version` requires every install command to pin a version. Clippy is a `rustup` component, not a package: `rustup component add clippy` cannot pin, and pinning `--toolchain=1.97.0` would ship a hardcoded Rust release to every user and go stale. Declaring none is a supported state (the install agent prompt already says "declares no install commands"). The cost is that doctor shows no fix line for a missing clippy. The README now states this rule.

    Not done, deliberately: `sah doctor` was not run against a rebuilt binary. The doctor facts were exercised through `check_review_engine_with`, which is the same function `sah doctor` consumes.
  timestamp: 2026-08-07T12:23:53.723848+00:00
- actor: claude-code
  id: 01kze2wg9m77es7j1rsxswn91j
  text: |-
    ### implement — changed
    - evidence: 12 files. Rules: `builtin/validators/code-hygiene/rules/missing-docs-rust.md`, `builtin/validators/code-hygiene/rules/missing-docs-python.md`. Fixtures: `builtin/validators/code-hygiene/fixtures/{Cargo.toml,Cargo.lock,lib.rs,missing-docs-rust.fail.rs,missing-docs-rust.pass.rs,missing-docs-python.fail.py,missing-docs-python.pass.py}`. Engine: `crates/swissarmyhammer-validators/src/doctor.rs` (per-fixture finding attribution), `crates/swissarmyhammer-validators/src/review/tool_rules.rs` (`normalize_tool_path` crate-visible + the two acceptance tests), `crates/swissarmyhammer-validators/src/builtin/mod.rs` (roster), `crates/swissarmyhammer-build/src/lib.rs` (`all_extensions()`), `crates/mirdan/build.rs` + `crates/mirdan/src/builtin_validators.rs` (embed the whole set + guards), `builtin/validators/README.md` (contract).
    - acceptance 1 (real-pipeline, zero LLM calls): `review::tool_rules::tests::the_shipped_rust_tool_rule_reports_an_undocumented_public_item` — loads the shipped builtins, plans the Rust tool rule healthy over a real cargo workspace, asserts `missing-docs` is suppressed for the file, runs the real clippy+jq pipeline, and gets exactly one CONFIRMED finding tagged `code-hygiene`/`missing-docs-rust`.
    - acceptance 2 (doctor fixtures): `review::tool_rules::tests::every_shipped_missing_docs_tool_rule_passes_its_fixtures` — runs `install_project_tool_rules` (the `sah init` pre-install) then asserts `check_review_engine_with` reports both rules `usable()`.
    - tests: `cargo test -p swissarmyhammer-validators -p mirdan -p swissarmyhammer-build` → 495 + 432 + 34 + doc-tests, 0 failed. `cargo clippy --workspace --all-targets -- -D warnings` → clean.
    - next: /review
  timestamp: 2026-08-07T12:24:06.708716+00:00
- actor: claude-code
  id: 01kze4bbhfn3m1bsf04gpsw7hj
  text: |
    ### review — findings
    - scope: `review sha HEAD~1..HEAD` (e1d4719cf)
    - evidence: 4 findings (4 confirmed, 2 refuted, 20 attempted) — builtin/validators/code-hygiene/fixtures/lib.rs:8, builtin/validators/code-hygiene/fixtures/lib.rs:11, crates/swissarmyhammer-validators/src/review/tool_rules.rs:121, crates/swissarmyhammer-validators/src/review/tool_rules.rs:174
    - deviation 1 (Rust rule declares no `install.commands`): accepted, no finding. `install_command_pins_version` (crates/swissarmyhammer-validators/src/review/tool_install.rs:442) requires a pinned version, and a rustup component has no package version to pin. The commit records the case in builtin/validators/README.md and in the rule prose. The fix-hint gap is on follow-up card ^s297bfh.
    - deviation 2 (Python rule uses `ruff check --isolated --no-cache`): accepted, no finding. The README clause about a temporary config file applies only when a tool needs a configuration file. `--isolated --select D1` gives ruff no configuration file to need, and it meets the stated intent: the script owns its whole invocation and never reads the project's own lint configuration.
    - next: fix the 4 findings, then re-review.
  timestamp: 2026-08-07T12:49:41.935546+00:00
- actor: claude-code
  id: 01kze4cc2nd0wqeq130hyt7mmr
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 12 files; the first two builtin tool rules with fixtures, plus two engine fixes (workspace-scope fixture attribution in doctor.rs, non-markdown builtin embedding in the mirdan build). Two deviations recorded and accepted by the review: no install.commands on the Rust rule (rustup components pin no version; follow-up ^s297bfh), and ruff --isolated in place of a temp config file.
    - test: green — cargo nextest run --workspace 13753 passed, doc tests 0 failed, fmt clean, clippy clean. The fixtures cargo package carries its own [workspace] table and does not join the main build.
    - commit: e1d4719cf
    - review: findings — builtin/validators/code-hygiene/fixtures/lib.rs:8, :11 (public modules with no doc comment), crates/swissarmyhammer-validators/src/review/tool_rules.rs:121, :174 (public error-like types with no Display)
  timestamp: 2026-08-07T12:50:15.253985+00:00
- actor: claude-code
  id: 01kze4tgq6btc6ze6yqd4zq5ba
  text: |
    Findings pass, notes for the next agent:

    1. **The two `lib.rs` findings are safe to fix, and I proved it.** The fixture harness declares both fixtures as `#[path]` modules. Clippy attributes a missing-doc on a module declaration to `lib.rs`, never to the fixture file, and `doctor::run_fixture` counts only the findings a run reports ABOUT the fixture under test. So documenting the two module declarations removes two `lib.rs` rows that the doctor already ignored, and changes neither fixture. Verified by running the rule's own pipeline in the fixtures directory after the edit: the whole package now reports exactly one row, `missing-docs-rust.fail.rs:9 missing documentation for a struct`. No fixture was weakened.

    2. **`ToolRunError` uses `thiserror`, `ToolFallback` uses a hand-written `Display`.** The finding on `ToolRunError` names `thiserror` itself, the crate already uses it for its error types (`src/error.rs`, `src/validators/pool.rs`), and the derive gives `Display` and `std::error::Error` together. `ToolFallback` is NOT an error — the review still ran, through the prompt rule — so it gets `Display` alone. A comment above the impl records that reason, so a later reader does not "unify" the two.

    3. **The report wording did not change.** `synthesize::render_tool_errors` and `render_tool_fallbacks` still write their own markdown blocks. They are not duplicates of `Display`: the error block is a multi-line blockquote of the raw stderr, and the fallback block also names the prompt rule that ran instead. Routing them through `Display` would drop that note and rewrite output the synthesize tests assert on.

    Both `Display` messages are lowercase with no trailing punctuation, per the `rust/error-handling` rule.
  timestamp: 2026-08-07T12:57:58.758408+00:00
- actor: claude-code
  id: 01kze4tv1jzss7tcnestb5ptnh
  text: |
    ### implement — changed
    - evidence: 2 files — builtin/validators/code-hygiene/fixtures/lib.rs (doc comment on each `#[path]` module declaration), crates/swissarmyhammer-validators/src/review/tool_rules.rs (`#[derive(thiserror::Error)]` + `#[error(...)]` on `ToolRunError`, hand-written `Display` for `ToolFallback`, 3 new tests).
    - findings: 4 of 4 closed, all four boxes now `- [x]`.
    - TDD: RED first — `cargo test -p swissarmyhammer-validators --lib review::tool_rules::tests::a_tool` failed to compile with 4 errors (`ToolRunError doesn't implement std::fmt::Display`, `ToolFallback doesn't implement std::fmt::Display`, and the `&dyn std::error::Error` coercion). GREEN after the impls.
    - tests: `cargo test -p swissarmyhammer-validators -p mirdan` → 498 + 432 + doc tests, 0 failed (1 pre-existing ignored doc example in mirdan/src/tool_install.rs). `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean. `cargo fmt --check` clean.
    - fixture proof: the rule's own pipeline in builtin/validators/code-hygiene/fixtures now reports exactly `missing-docs-rust.fail.rs:9`; both `every_shipped_missing_docs_tool_rule_passes_its_fixtures` and `the_shipped_rust_tool_rule_reports_an_undocumented_public_item` still pass.
    - next: /review
  timestamp: 2026-08-07T12:58:09.330964+00:00
- actor: claude-code
  id: 01kze5mbbkjetms3kkr1fs2eet
  text: |
    ### review — clean
    - scope: review sha HEAD~1..HEAD (1d279810b)
    - evidence: 0 findings; 9 pairs attempted, 0 failed, 0 skipped
    - prior findings verified closed:
      - builtin/validators/code-hygiene/fixtures/lib.rs — `fail` and `pass` modules each carry a doc comment
      - crates/swissarmyhammer-validators/src/review/tool_rules.rs:170 — hand-written `Display` for `ToolFallback`
      - crates/swissarmyhammer-validators/src/review/tool_rules.rs:187 — `ToolRunError` derives `thiserror::Error`, so it has `Display` and `std::error::Error`
    - doctor fixture checks after the fixture lib.rs edit: `cargo test -p swissarmyhammer-validators --lib` — 4 passed, 0 failed. `every_shipped_missing_docs_tool_rule_passes_its_fixtures` asserts `row.usable()` for both `missing-docs-rust` and `missing-docs-python`, which requires `FixtureOutcome::Passed` for each pair.
    - `check validators`: ok, 14 validators, 0 errors
    - next: task moved to done
  timestamp: 2026-08-07T13:12:05.235751+00:00
- actor: claude-code
  id: 01kze5n1r2yvdvzhfqfrw1sx9v
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 2 files; the 4 prior findings are closed and verified in the source
    - test: green — cargo nextest run --workspace 13756 passed, doc tests 0 failed, fmt clean, clippy clean
    - commit: 1d279810b
    - review: clean — 0 findings, 9 pairs attempted. The doctor fixture checks still pass for both shipped tool rules; check validators reports ok across 14 validators with 0 errors. Task moved to done.
  timestamp: 2026-08-07T13:12:28.162217+00:00
depends_on:
- 01KZ9356Y8XTJ6A28KQCBNFE97
- 01KZ935GJX1YS2EAD7C2HK89AJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffc080
title: missing-docs runners for Rust and Python, with fixtures
---
Ship the first two tool rules for missing docs, in the `code-hygiene` set.

Both are rule files in `builtin/validators/code-hygiene/rules/` with a `tool` block and `supersedes: missing-docs`. The prompt rule `missing-docs.md` stays unchanged as the fallback and as the rule for languages with no tool rule yet.

Rust tool rule (`missing-docs-rust.md`):
- match: files `**/*.rs`, project_types [rust]. `tool.scope: workspace`.
- run: `cargo clippy --message-format=json -- -W missing_docs` piped through `jq -c` to select diagnostics with code `missing_docs` and emit `{file, line, message}` lines. The pipe is the whole mapping and the whole filter.
- Engine keeps only findings in changed files (workspace scope).
- Doctor: `which cargo-clippy jq`. Install: `rustup component add clippy`.

Python tool rule (`missing-docs-python.md`):
- match: files `**/*.py`, project_types [python]. `tool.scope: files`.
- run: `ruff check --select D1 --output-format json "$@"` piped through `jq -c` to emit `{file, line, message}` lines.
- Doctor: `which ruff jq`. Install: pinned ruff via uv / pipx / brew.

Both:
- Test each pipeline in a terminal first; the frontmatter holds exactly that pipeline.
- Ship `fixtures/<name>.fail.<ext>` (one undocumented public item) and `fixtures/<name>.pass.<ext>` (fully documented).
- Exemptions live in tool config or inline suppressions, not prose.

Acceptance:
- Real-pipeline test on this repo: `review working` with an undocumented pub item reports it from the Rust tool rule, with zero LLM calls for that pair.
- Fixture checks pass in doctor for both tool rules.

#tool-validators

## Review Findings (2026-08-07 07:36)

- [x] `builtin/validators/code-hygiene/fixtures/lib.rs:8` — Public module `fail` has no doc comment. All public items must be documented. Add a doc comment above line 8, e.g. `/// The fail fixture module.` or similar explaining its purpose.
- [x] `builtin/validators/code-hygiene/fixtures/lib.rs:11` — Public module `pass` has no doc comment. All public items must be documented. Add a doc comment above line 11, e.g. `/// The pass fixture module.` or similar explaining its purpose.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:121` — ToolFallback is a public type representing an error-like fallback condition but does not implement Display trait. Like ToolRunError, it represents a failure mode and should support standard error formatting. Add `impl Display for ToolFallback` to format the fallback details: `impl Display for ToolFallback { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "tool rule `{}` in validator `{}` fell back: {}", self.rule, self.validator, self.detail) } }`.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:174` — ToolRunError is a public error type but does not implement Display trait. Error types must implement Display to support standard error formatting and reporting patterns. Add `impl Display for ToolRunError` to format the error details: `impl Display for ToolRunError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "tool rule `{}` in validator `{}` broke: {}", self.rule, self.validator, self.detail) } }`. Consider also implementing `std::error::Error` for complete error type support, or use the `thiserror` crate for automatic implementations.
