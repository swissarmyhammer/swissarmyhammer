---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kztcx0s9jsfm7xkmgf2vdah4
  text: |-
    Research, measured 2026-08-12 with the shipped scripts extracted out of each rule and run over the card's probe crate (`pub struct Undocumented` with no closing semicolon, cargo 1.97.1, clippy 0.1.97, cargo-machete 0.9.2):

    | rule | findings | exit | reads |
    |---|---|---|---|
    | `complexity-rust` | 0 | **1** | `complexity-rust: cargo clippy could not lint the workspace` on stderr |
    | `dead-code-rust` | 0 | 0 | cargo's `could not compile` on stderr, nothing else |
    | `unused-dependencies-rust` | its findings | 0 | machete does not read the compiler at all |

    So `complexity-rust` no longer holds the defect. Card ^w5v73k1 removed it after this card was written (commits 002f8c54b, 6b228de75, eab5cc608, a4cd1eee9, ecc9417b1 on 2026-08-11 evening; this card was written that morning). Its rule body states the measurement in "A workspace the tool cannot lint", the row `a package that does not parse: pub fn broken( { | 101 | ... | 0 findings, exit 1`, and the acceptance test `the_shipped_rust_complexity_tool_rule_breaks_on_a_workspace_it_cannot_compile` holds it. Nothing to change there; re-writing it into the simpler `missing-docs-rust` shape would take the deny-level rows back out.

    `unused-dependencies-rust` does not break on a crate that does not compile: machete parses with `syn` and answers the same list for a source file that does not parse. Measured on a probe that names `libc` and drops its closing brace: `didn't find any unused dependencies`, exit 0, the same as the control that parses. The defect it DOES hold is the same class the card names — the pipe drops machete's status. Measured statuses of machete 0.9.2:

    | shape | status | stdout | stderr |
    |---|---|---|---|
    | findings | 1 | `cargo-machete found the following unused dependencies` | `Analyzing…` / `Done!` |
    | clean | 0 | `didn't find any unused dependencies` | the same |
    | a path that holds no file | 2 | nothing | `Error: Errors when walking over directories` |
    | a manifest that does not parse as TOML | **0** | `didn't find any unused dependencies` | `error when handling <path>: TOML parse error…` |
    | a detached workspace member | **0** | the same | `error when handling <path>: can't load root workspace` |
    | the bare name `Cargo.toml` | **0** | the same | `error when handling Cargo.toml: can't load root workspace at :` |

    Three broken shapes exit 0 and write the clean sentence, so the status alone cannot carry them; the `error when handling ` line on stderr is what tells them apart.

    `cargo check` in `dead-code-rust` shares its nonzero status between a broken run and a MEASURED run, the same way `cargo clippy` does in `complexity-rust`: a lint at deny level, or `RUSTFLAGS="-D warnings"`, makes cargo exit 101 for a workspace it checked from end to end — and under `-D warnings` the `dead_code` findings themselves arrive at level `error`. So the simple `set -e` shape would break the run exactly when the rule has findings. `builtin/validators/README.md` states the answer for a shared status: test the REPORT beside the status. `dead-code-rust` therefore takes the four raw-report tests `complexity-rust` carries, each measured again for `cargo check`.
  timestamp: 2026-08-12T07:10:02.537620+00:00
- actor: claude-code
  id: 01kztfnyfmdw2s68qgyphkmy24
  text: |-
    Implementation landed.

    **`dead-code-rust`** — the script now writes `cargo check` into a file and tests the RAW report beside the status, the shape `complexity-rust` carries. RED to GREEN with the real tool over each shape:

    | the shape | before | after |
    |---|---|---|
    | a crate that does not parse | 0 findings, exit 0 | 0 findings, exit 1, `cargo could not compile the workspace` |
    | a workspace member that does not parse beside a dirty member | the good member's finding, exit 0 | 0 findings, exit 1 |
    | a build script that breaks | 0 findings, exit 0 | 0 findings, exit 1, the build-script line |
    | the same package under a build script that runs | 1 finding, exit 0 | unchanged |
    | `[lints.rust] unused_variables = "deny"` beside one dead item | 1 finding, exit 0 | unchanged |
    | `RUSTFLAGS="-D warnings"` beside one dead item | 1 finding, exit 0 | unchanged |
    | `jq` replaced by a command that exits 127 | the orphan half alone, exit 0 | 0 findings, exit 1 |
    | this whole workspace | 0 findings, exit 0 | the same bytes, exit 0 |

    The two `deny` rows are why the simple `set -e` shape was refused: under `-D warnings` a `dead_code` diagnostic itself arrives at level `error` and cargo exits 101, so a status-only gate would break the run exactly when the rule has a finding. Six acceptance tests hold the table.

    **`unused-dependencies-rust`** — machete's status and stderr are now read for each manifest. Measured RED to GREEN: a manifest that does not parse as TOML answered 0 findings at exit 0 (machete writes the clean sentence to stdout and `error when handling` to stderr) and now exits 1; a machete a machine cannot run answered 0 findings at exit 0 and now exits 1 naming the status. The whole-workspace answer is unchanged: the same 126 findings across 37 packages, exit 0, 2 s.

    A second defect fell out of that work and is fixed here too: `cargo machete <path>` reads the subcommand name as a PATH of its own when `CARGO_PKG_NAME` stands in the environment, which cargo exports to every process it runs. Measured with machete 0.9.2: the run then walks `machete,<path>`, writes its findings, and exits 2. The script now runs `cargo-machete` and `doctor.check_version_command` reads `cargo-machete --version`, which answer the same in either environment. The acceptance tests found it, because a test binary carries `CARGO_PKG_NAME`.

    **`complexity-rust`** — no change. Measured over the card's own probe crate with the shipped script today: 0 findings, exit 1, `complexity-rust: cargo clippy could not lint the workspace`. Card ^w5v73k1 removed the defect after this card was written, its rule body states the measurement, and `the_shipped_rust_complexity_tool_rule_breaks_on_a_workspace_it_cannot_compile` holds it. Rewriting it into the simpler shape would take the deny-level rows back out.

    Two findings of the test work, each a card of its own:

    - ^btphzkn — `missing-docs-rust` holds the mirror defect: it tests the status alone, so a workspace clippy linted from end to end, carrying a lint at deny level, breaks the run and its undocumented item goes unreported. Measured: exit 101, 0 findings.
    - ^fpg9823 — `the_swift_package_root_restores_the_directory_before_it_removes_it` fails when the suite runs in parallel and passes alone, on this branch and on the unchanged tree alike.

    One test-harness defect was fixed on the way: the probe that stubs a command on `PATH` used to break every OTHER test that ran a shipped script in that window. Measured with the plain stub: 8 failures over the tool-rule suite, four `complexity-go` fixture pairs on `exit status: 127` and three `complexity-rust` tests on `jq could not read the clippy report`. The stub now exits nonzero only when the probe's own marker file stands in the working directory, and it stands in one directory that outlives the test, so a run that read the stubbed `PATH` still finds a command there.
  timestamp: 2026-08-12T07:58:36.532867+00:00
- actor: claude-code
  id: 01kztfp61kanap6sc66w6kb8g1
  text: |-
    ### implement — changed
    - evidence: 7 files — builtin/validators/code-hygiene/rules/dead-code-rust.md, builtin/validators/manifests/rules/unused-dependencies-rust.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs, .../tests/shipped.rs, .../tests/shipped/dead_code_rust.rs (new), .../tests/shipped/complexity_rust.rs, .../tests/shipped/unused_dependencies.rs. `cargo test -p swissarmyhammer-validators`: 679 passed, 1 failed — `the_swift_package_root_restores_the_directory_before_it_removes_it`, which fails the same way on the unchanged tree (^fpg9823). `cargo fmt --check` and `cargo clippy --all-targets`: clean.
    - next: /review
  timestamp: 2026-08-12T07:58:44.275489+00:00
- actor: claude-code
  id: 01kztgafw0vm4xy2599p5w2ypr
  text: |-
    ### commit — changed
    - evidence: f495f760c fix(validators): exit nonzero from dead-code-rust and unused-dependencies-rust on a crate that does not compile (^y4xyw1g)
    - next: /review
  timestamp: 2026-08-12T08:09:49.696387+00:00
- actor: claude-code
  id: 01kztgv9jnsdb3jra8pebaj1ts
  text: |
    ### review — findings
    - evidence: 2 findings (2 confirmed, 6 refuted, 8 attempted) over `review sha HEAD~1..HEAD` (f495f760c) — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs:85, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs:170
    - next: remove the unnecessary `#[serial_test::serial(env)]` markers this commit added; task stays in `review`
  timestamp: 2026-08-12T08:19:00.309616+00:00
- actor: claude-code
  id: 01kztgvxhmsjhcwans3chje62c
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 7 files (dead-code-rust.md, unused-dependencies-rust.md, 5 test files)
    - test: red — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3443 run, 3439 passed, 4 failed. All 4 reproduce on the unchanged tree with the diff stashed, and all 4 are already carded on ^bh5ncd0. This change caused none of them. fmt and clippy clean.
    - commit: f495f760c932e27ff9557218fa0fc43f696abebb
    - review: findings — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs:85, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs:170
  timestamp: 2026-08-12T08:19:20.756162+00:00
- actor: claude-code
  id: 01kzth1pvyzvdgf5kmc50n2a1c
  text: |-
    Research for the two open findings.

    `#[serial_test::serial(env)]` puts a test in the `env` exclusion group. It earns its place only when the test writes process state, or when it reads process state another `env` test writes. Read every marker in the commit against that:

    | file:test | writes process state | keeps the marker |
    |---|---|---|
    | `unused_dependencies.rs:86` reports an unused dependency | none | no |
    | `unused_dependencies.rs:171` breaks on a manifest it cannot read | none | no |
    | `unused_dependencies.rs:216` breaks when machete cannot run | `PathGuard::prepend` | yes |
    | `complexity_rust.rs:786` measures a workspace beside deny-level flags | `EnvVarGuard::set(RUSTFLAGS)` | yes |
    | `complexity_rust.rs:841` breaks when the filter cannot read the report | `PathGuard::prepend` | yes |
    | `dead_code_rust.rs` six tests | one prepends `PATH`; the other five run `cargo check` | yes — see below |

    The two flagged tests cannot read either kind of state:

    - The `PATH` stub `verify_shipped_tree_breaks_without` writes execs the real binary unless `.sah-broken-command` stands in the working directory. `run_script_findings` gives each probe its OWN temporary directory as the child's working directory (`run_shell(script, Some(dir), args)`), and neither flagged probe stages that file. So a stubbed `PATH` hands both of them the real tool.
    - `RUSTFLAGS` is the other process state an `env` test writes. Both flagged tests drive `cargo-machete`, which parses with `syn` and never invokes the compiler, so `RUSTFLAGS` cannot reach them.

    `dead_code_rust.rs` keeps its six markers: each of those tests runs `cargo check`, and `RUSTFLAGS="-D warnings"` set by `complexity_rust.rs:786` would raise their `dead_code` findings to level `error` and make cargo exit 101. Its module docstring names only the `PATH` stub as the reason, which the marker-file guard has since answered; the compiler flag is the reason that stands.
  timestamp: 2026-08-12T08:22:30.526210+00:00
- actor: claude-code
  id: 01kzth802qw1yg895v5xnr7ctr
  text: |-
    Both findings answered in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs`.

    - Removed `#[serial_test::serial(env)]` from `the_shipped_rust_unused_dependency_tool_rule_reports_an_unused_dependency`.
    - Removed `#[serial_test::serial(env)]` from `the_shipped_rust_unused_dependency_tool_rule_breaks_on_a_manifest_it_cannot_read`.
    - Kept it on `the_shipped_rust_unused_dependency_tool_rule_breaks_when_machete_cannot_run`, the third marker the commit added and the one test of the module that writes process state. Its docstring now states why it alone carries the marker, so the next author does not put the marker back on its neighbours.

    Measured after the change, with the two tests running in parallel again:

    | run | result |
    |---|---|
    | `cargo nextest run -p swissarmyhammer-validators -E 'test(/tool_rules::tests::shipped/)'` | 116 run, 116 passed, 0 failed |
    | the same filter narrowed to `unused_dependencies` + `complexity_rust` + `dead_code_rust`, five times over | 24 run, 24 passed, each time |
    | `cargo fmt --check` | clean |
    | `cargo clippy --workspace --all-targets -- -D warnings` | clean |

    Five runs, because the shape the marker guarded against is a race: the `cargo-machete` stub and the two tests now run at the same time. They pass every time, which is what the code says they must — the stub execs the real binary unless `.sah-broken-command` stands in the child's working directory, and each probe gets a temporary directory of its own.
  timestamp: 2026-08-12T08:25:56.567093+00:00
- actor: claude-code
  id: 01kzth85q3ngsvefp3qpw6vsrw
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs. `cargo nextest run -p swissarmyhammer-validators -E 'test(/tool_rules::tests::shipped/)'`: 116 run, 116 passed, 0 failed. The narrowed filter, five runs: 24 passed each time. `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`: clean. Both review findings checked.
    - next: /review
  timestamp: 2026-08-12T08:26:02.339985+00:00
- actor: claude-code
  id: 01kztht3f9zhcwy24g39sc2eat
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (0142cffbc) — 0 findings, 0 confirmed, 0 refuted, 8 validator runs attempted, 0 failed. Both findings of the round of 2026-08-12 03:10 are checked.
    - next: none. The task moves to done.
  timestamp: 2026-08-12T08:35:49.865922+00:00
- actor: claude-code
  id: 01kzthtnv6rft8t3nky11fjhgh
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 1 file (tests/shipped/unused_dependencies.rs), both prior findings checked
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3439 passed. The 4 remaining failures are pre-existing and carded on ^bh5ncd0. Run 3 times to prove the dropped serial(env) markers add no race. fmt and clippy clean.
    - commit: 0142cffbc
    - review: clean — 0 findings over HEAD~1..HEAD, task moved to done
  timestamp: 2026-08-12T08:36:08.678879+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffea80
title: Three Rust tool rules answer zero and exit 0 for a crate that does not compile
---
Found by the measurement on ^302hw8c. That card removed the defect from `missing-docs-rust.md`. Three Rust rules still hold it.

## The defect

`cargo` exits 101 for a crate that does not compile. A shell pipeline takes the exit status of its LAST command, and each of these rules ends its cargo pipeline in `jq`, `sort` or `awk`. The pipeline therefore exits 0 with no output, and the engine reads exit 0 as "the tool judged the code". A crate that never compiled reads as a clean crate.

`builtin/validators/README.md` states the requirement under the `run` key: "Write a pipe only where the tool cannot exit nonzero. Otherwise write a script: run the tool into a file, test the status, and exit nonzero yourself." Cargo has a failure status of its own, so a pipe is not safe for it.

## Measured on 2026-08-11

Each script was read out of its shipped rule and run in a probe crate whose `src/lib.rs` holds `pub struct Undocumented` with no closing semicolon. `cargo` exits 101 in that crate.

| rule | findings | exit |
|---|---|---|
| `complexity-rust` | 0 | 0 |
| `dead-code-rust` | 0 | 0 |
| `unused-dependencies-rust` | 0 | 0 |

`cargo-machete` was installed for the third measurement (`/Users/wballard/.cargo/bin/cargo-machete`).

`missing-docs-rust` is the one Rust rule that breaks. It holds:

```
set -e
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cargo clippy --workspace --message-format=json --quiet -- -W missing_docs > "$work/clippy.json"
jq -c '...' "$work/clippy.json" | sort -u
```

Measured over the same probe crate: it reports no finding and exits 101, with cargo's own error on stderr.

## What to do

- Give each of the three rules the shape above: run cargo into a file, and let `set -e` make cargo's exit status the exit status of the script.
- Prove each behaviour change RED to GREEN with the real tool. Reuse `ShippedBrokenRun` and `verify_shipped_run_breaks` in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`. ^302hw8c added the `support` field those shapes need for a `workspace`-scope probe, and `the_shipped_rust_missing_docs_tool_rule_breaks_on_a_crate_that_does_not_compile` is the worked example.
- State the measurement in each rule body.

#tool-validators #objectivity

## Review Findings (2026-08-12 03:10)

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs:85` — The `#[serial_test::serial(env)]` marker was added to `the_shipped_rust_unused_dependency_tool_rule_reports_an_unused_dependency()`, but this test does not call `verify_shipped_tree_breaks_without()` or any other function that modifies the `PATH` environment variable. The marker should only be on tests that modify process environment state (per the docstring at shipped.rs:11-16, which explains this is a guard against one test's PATH stub interfering with another's). Adding it here serializes a test unnecessarily. Remove the `#[serial_test::serial(env)]` attribute from line 85. Only the test at line 216 (which calls `verify_shipped_tree_breaks_without()`) should bear this marker.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/unused_dependencies.rs:170` — The `#[serial_test::serial(env)]` marker was added to `the_shipped_rust_unused_dependency_tool_rule_breaks_on_a_manifest_it_cannot_read()`, but this test calls `verify_shipped_tree_breaks()` (line 172), not `verify_shipped_tree_breaks_without()`. The former does not modify PATH, so the serial marker is unnecessary. The marker should only be on tests that actually modify process environment state. Remove the `#[serial_test::serial(env)]` attribute from line 170.
