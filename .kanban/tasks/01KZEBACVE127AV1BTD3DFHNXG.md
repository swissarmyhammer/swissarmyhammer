---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzgs88p6z4rgvchbm90pye75
  text: |-
    Research done. Facts measured on this machine (ruff 0.14.5, clippy 0.1.97).

    CLIPPY_CONF_DIR verified, three ways:
    - With `cognitive-complexity-threshold = 1000` / `too-many-lines-threshold = 1000` in the conf dir, both lints go silent on a probe crate that trips them at 15/250. So clippy reads the file.
    - With `clippy.toml` in the package saying 1000 AND CLIPPY_CONF_DIR saying 15/250, both lints fire. So the environment variable wins and the project file is never read.
    - A cached second and third run re-emit both warnings, so a repeat review still reports.

    The repository has no clippy.toml of its own today.

    `bash -c` runs the script, so `trap 'rm -rf "$conf"' EXIT` cleans the temporary conf dir and leaves the pipe's exit code as the script's exit code.

    Python statement threshold, measured not guessed. Ran `ruff --select PLR0915 --config lint.pylint.max-statements=0` over the CPython 3.12 standard library; the message carries each function's exact ruff statement count. Compared against code lines (blank and comment-only excluded) computed with `ast`:
    - 60 functions of 80 code lines or more: median 0.732 statements for each code line.
    - 22 functions of 120 or more: median 0.728.
    - 8 functions of 150 or more: median 0.722.
    250 code lines x 0.72 = 180. The rule sets `lint.pylint.max-statements=180`.

    Engine facts that shape the rules:
    - `run_shell` is `bash -c <script> bash <args>`; no `set -e`, no pipefail, no environment injected. cwd is the repository root for both scopes in a real review, and the fixture scratch dir when doctor runs.
    - Workspace-scope findings are kept by exact repo-relative string match; fixture attribution is by base name, so one cargo package may hold every Rust fixture.
    - `ToolSpec` is `deny_unknown_fields`.

    Three test rosters enumerate the shipped tool rules and each needs the new rules: `builtin/mod.rs` (count assertion), `review/tool_rules.rs` (`SHIPPED_*` consts plus the fixture acceptance test), `crates/mirdan/src/builtin_validators.rs` (embedded fixture file names).
  timestamp: 2026-08-08T13:33:29.670381+00:00
- actor: claude-code
  id: 01kzgt6znkq2w84fc4n2parngg
  text: |-
    Implementation landed and green, but one half of the Rust plan is blocked by a measurement. Detail below.

    ## What is done and green

    Three rules, six fixtures, three test rosters updated. `cargo fmt --check` and `cargo clippy --all-targets` are clean on both changed crates.

    - `rules/complexity-rust.md` — workspace scope, one clippy run, temporary `clippy.toml` through `CLIPPY_CONF_DIR`, `supersedes: [cognitive-complexity, function-length]`.
    - `rules/complexity-python.md` — ruff `C901` at `max-complexity=15`, supersedes `cognitive-complexity`.
    - `rules/function-length-python.md` — ruff `PLR0915` at `max-statements=180`, supersedes `function-length`.
    - Six `.tmpl` fixtures, plus `lib.rs.tmpl` and `Cargo.toml.tmpl` updated so the fixture package holds all four Rust fixture modules.
    - `builtin/mod.rs`: new `CODE_HYGIENE_COMPLEXITY_TOOL_RULES` roster. It is the first roster whose `supersedes` differs per row, so the rows carry their own lists.
    - `tool_rules.rs`: the `SHIPPED_*` rosters now carry `supersedes` per row and `verify_shipped_tool_rules_pass_fixtures` lost its group-wide parameter. New `every_shipped_complexity_tool_rule_passes_its_fixtures`, and a new end-to-end test `the_shipped_rust_complexity_tool_rule_reports_an_over_complex_function`.
    - `crates/mirdan/src/builtin_validators.rs`: six fixture names added to the embed roster.

    ## RED verified six ways, then GREEN restored

    The fixture pair is not self-confirming, so each half was broken on purpose and the test watched to fail:

    1. Rust thresholds raised to 1000/10000 — "the fail fixture complexity-rust.fail.rs.tmpl produced no findings".
    2. `complexity-python` raised to 1000 — same message for its fail fixture.
    3. `function-length-python` raised to 10000 — same message for its fail fixture.
    4. Rust thresholds lowered to 1/5 — "the pass fixture complexity-rust.pass.rs.tmpl produced 5 finding(s); none are allowed".
    5. `- function-length` deleted from the Rust supersedes list — the end-to-end test fails with "`function-length` is missing from {cognitive-complexity, missing-docs}".
    6. `CLIPPY_CONF_DIR="$conf"` removed from the run — the rule plans no run at all, because at clippy's defaults the pass fixture reports. This proves the temporary config reaches clippy.

    ## BLOCKER: `clippy::cognitive_complexity` counts macro-expanded code

    `clippy::too_many_lines` is fit. `clippy::cognitive_complexity` is not, and the card's Rust rule requires both.

    Ran the rule's own script over this workspace: 442 findings. 6 are `too_many_lines` (3 of those in generated `target/.../out/*.rs`, which the engine drops). The other 436 are `cognitive_complexity`.

    The 436 are not real. `load_builtins` in `crates/swissarmyhammer-validators/src/builtin/mod.rs` is twelve lines — one `for`, two `if let Err`, two `tracing` calls — and clippy scores it 18 against the gate of 15.

    Isolated the cause with a two-crate probe. Same function shape, one loop, two `if let Err`, two format-args `tracing` calls:

    - `tracing` without the `log` feature: score 5.
    - `tracing` with the `log` feature: score 18 — the exact number this workspace reports for `load_builtins`.

    `cargo tree -e features` confirms this workspace builds `tracing` with `log` on. Clippy walks the macro expansion, and the `log` bridge expands each call into many branches. The noise is proportional to how many times a function logs, so no threshold separates it from real branching, and the pipe cannot filter it — the finding carries only a function and a number.

    Under the README contract a tool finding is a requirement, so shipping this half makes 436 `#[allow]` suppressions or rewrites mandatory across the repository. Worse, it would supersede the `cognitive-complexity` prompt rule, replacing an accurate Sonar measurement with a macro-expansion artifact.

    ## Why this stops rather than self-corrects

    The fix that fits the evidence is to drop `-W clippy::cognitive_complexity`, keep `too_many_lines`, rename the rule `function-length-rust`, and let Rust keep the `cognitive-complexity` prompt rule and the `complexity` probe — the card's own stated fallback, and the same verdict `VALIDATOR.md` already records for Rust and `magic-numbers`.

    That is not mine to decide. The card states the command and `supersedes: [cognitive-complexity, function-length]` outright, and its dependency ^gwhwyt8 was built so one clippy run could supersede two prompt rules. Dropping the lint removes this card's only production use of that feature and removes the reason the dependency exists. A person decides that.

    The tree is left green and one edit from either outcome: keep the rule as written, or delete one `-W` flag and one `supersedes` name and rename the rule and its two fixtures.

    ## Unrelated observation

    `./crates/swissarmyhammer-mcp-proxy/.kanban` exists. Nothing in this task made it. Not touched.
  timestamp: 2026-08-08T13:50:16.243152+00:00
- actor: claude-code
  id: 01kzgt83d2jdy02s2147bapmsz
  text: |-
    ### implement — stuck
    - evidence: 15 files — 3 new rules, 6 new fixtures, 3 modified fixture support files, `crates/swissarmyhammer-validators/src/builtin/mod.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules.rs`, `crates/mirdan/src/builtin_validators.rs`, `builtin/validators/code-hygiene/VALIDATOR.md`. Tests: 32 passed in `builtin::`, 24 passed in `review::tool_rules::`, 7 passed in mirdan `builtin_validators`. `cargo fmt --check` and `cargo clippy --all-targets` clean on both crates. RED verified six ways before GREEN.
    - next: a person picks one of the two options in the `## Blocker` section of the description. `clippy::cognitive_complexity` counts macro-expanded `tracing` calls and reports 436 findings in this workspace, so the Rust rule cannot supersede `cognitive-complexity` as the card states.
  timestamp: 2026-08-08T13:50:52.834013+00:00
- actor: claude-code
  id: 01kzgvetdndzy6rhk7xxd97nqr
  text: |-
    ### Blocker resolved — a better clippy lint exists

    A lint survey enumerated the full clippy 0.1.97 lint list (1114 lines, every group) instead of trusting the two lints this card names. Result: `clippy::excessive_nesting` is fit and uncontaminated, so Rust keeps a real deterministic complexity gate. The card's fallback to the prompt rule is NOT needed.

    Probe: two crates, byte-identical `src/lib.rs`, one with `tracing` `default-features = false`, one with the `log` feature on.

    | Lint | Group | Contaminated by `tracing`+`log`? | Workspace findings | Fit |
    |---|---|---|---|---|
    | `cognitive_complexity` | restriction | YES — same fn 6 -> 18; a flat zero-branch fn with 6 log calls 7 -> 43 | 460 at 15 | UNFIT |
    | `excessive_nesting` | complexity (warn) | NO — identical spans at thresholds 1-7 | 55 at 6 | FIT |
    | `too_many_lines` | pedantic | NO — 13/12/7/2/2 both crates | 4 real source at 250 | FIT |
    | `too_many_arguments` | complexity (warn) | NO | 0 at 7 | FIT |
    | `type_complexity` | complexity (warn) | NO | 0 at 250 | FIT |

    Clippy 0.1.97 has no cyclomatic lint and no source-level cognitive lint. `large_stack_frames` is contaminated and measures stack bytes, not complexity.

    `excessive_nesting` semantics, measured: every lexical block is one level, including inline `mod`, `impl`, and fn body. Only the outermost offending block of a chain reports, so a pyramid gives one finding. `#[allow]` on a function suppresses it. Item overhead means an `impl` method carries 2 levels, so control-flow depth D trips when D + 2 > T.

    Threshold 6 chosen by sampling: T=5 gives 199 findings and flags readable code; T=7 gives 19 but lets 5-deep pyramids through; T=6 gives 55 findings across 41 functions in 28 files, every sample genuine.

    The claim in `complexity-rust.md` line 71 — "The prompt rule's second gate, condition-nesting depth 4 or more, has no clippy lint, so superseding drops it for Rust" — is measurably false and must be deleted.

    Two script fixes measured as necessary: `sort -u` because `--all-targets` lints lib and test targets and emitted 129 lines for 63 distinct findings, and a filter dropping absolute paths because `too_many_lines` fires on four generated `OUT_DIR` files.

    Do NOT use `--force-warn`. These lints are warn-by-default and 36 legitimate inline `#[allow]`s exist. No crate-level `#![allow]` exists for any of them.
  timestamp: 2026-08-08T14:12:01.589017+00:00
- actor: claude-code
  id: 01kzgvp1ntx0j9k5pefp81h46y
  text: |-
    Picked up again after the human decision. Working the settled `## Decision (settled)` section: four adopted lints, `clippy::cognitive_complexity` dropped.

    Measured `clippy::excessive_nesting` semantics on a fresh probe crate before editing, at `excessive-nesting-threshold = 6`:

    - Free function, control-flow depth 1..8: findings at depth 6, 7 and 8. Depth 5 is silent.
    - `impl` method, control-flow depth 1..8: findings at depth 5, 6, 7 and 8. Depth 4 is silent.

    So a block reports when its nesting level is over the threshold, and the level is the control-flow depth plus the item overhead — 1 for a free function body, 2 for an `impl` method. This is the arithmetic the card states, confirmed on this machine with clippy 0.1.97.

    Only one finding for each chain: depth 7 and depth 8 each report one block, the sixth, not every block below it.
  timestamp: 2026-08-08T14:15:58.394004+00:00
- actor: claude-code
  id: 01kzgw4mgb211ts5ptxq07peqd
  text: |-
    Rust half rebuilt on the settled decision. Green.

    ## What changed

    - `rules/complexity-rust.md` — rewritten. The run is the card's verified script word for word: temporary `clippy.toml` through `CLIPPY_CONF_DIR`, `--workspace --all-targets`, the four `-W` flags, the four-code `jq` filter, the absolute-path drop, `sort -u`. Name and `supersedes: [cognitive-complexity, function-length]` unchanged. The body now says plainly that the rule gates lexical nesting depth, not a cognitive score, and gives the item-overhead arithmetic: free function `D + 1 > T`, `impl` method `D + 2 > T`. The false claim that the nesting gate has no clippy lint is gone.
    - `fixtures/complexity-rust.fail.rs.tmpl` — `classify_reading` replaced by `fold_grid`, a free function at control-flow depth 6, so the innermost block sits at nesting level 7. `mix_state` at 262 lines is unchanged. The fixture now trips BOTH gates.
    - `fixtures/complexity-rust.pass.rs.tmpl` — the same two shapes under the gates: `fold_grid` at control-flow depth 5, so level 6, at the gate and not over it; `mix_state` at 202 lines.
    - `VALIDATOR.md` — the complexity section now names the four lints and states that Rust keeps the nesting gate while Python drops it. The three rejected-tool verdicts moved out of the dead-code section into a new `## Tools measured and rejected` section, and `clippy::cognitive_complexity` joins them as the fourth, with the probe evidence: a flat zero-branch function with six `tracing` calls scores 7 without the `tracing` `log` feature and 43 with it, and 460 findings workspace-wide.
    - `review/tool_rules.rs` — the end-to-end probe `COMPLEX_LIB_RS` was a cognitive-complexity shape and reported nothing under the new lint set. It is now a depth-6 `fold_grid`, and the claim fragment moved from `cognitive complexity` to `too nested`.

    The three rosters needed no edit. The rule name and both fixture file names are unchanged, so `builtin/mod.rs`, the `SHIPPED_COMPLEXITY_RULES` roster and the mirdan embed roster already name them. Their tests confirm it.

    The Python half was not touched.

    ## Direct script evidence

    Materialized the whole fixtures directory the way the doctor does and ran the rule's script over it:

    - Both gates on: `complexity-rust.fail.rs` line 26 `this block is too nested`, line 42 `this function has too many lines (262/250)`. Nothing from the pass fixture.
    - Line gate raised to 10000: only the nesting finding remains.
    - Nesting gate raised to 1000: only the line finding remains.

    So each gate fires on the fail fixture on its own.

    ## RED verified five ways, then GREEN restored

    Each break ran the two complexity tests and each failed with the expected message.

    1. Both gates raised out of reach — "the fail fixture complexity-rust.fail.rs.tmpl produced no findings; at least one is required".
    2. Nesting gate lowered 6 -> 5 — "the pass fixture complexity-rust.pass.rs.tmpl produced 1 finding(s); none are allowed". The pass pyramid sits at level 6 exactly, so one step tips it.
    3. Line gate lowered 250 -> 200 — the same pass-fixture message, one finding. The pass `mix_state` runs 202 lines.
    4. `CLIPPY_CONF_DIR` dropped from the run — the pass fixture reports and the rule plans no run at all. `excessive-nesting-threshold` defaults to `0`, which turns the lint off, so a run that reports at all proves the temporary file reached clippy.
    5. `- function-length` deleted from `supersedes` — "`function-length` is missing from {cognitive-complexity, missing-docs}".

    The rule file was restored byte for byte after each break.

    ## GREEN

    - `swissarmyhammer-validators` lib: 526 passed, 0 failed.
    - `mirdan` `builtin_validators`: 7 passed.
    - `cargo fmt --check` and `cargo clippy --all-targets` clean on both crates.
  timestamp: 2026-08-08T14:23:56.427982+00:00
- actor: claude-code
  id: 01kzgw52afwj1szmc9ajdvwn5s
  text: |-
    ### implement — changed
    - evidence: 5 files — `builtin/validators/code-hygiene/rules/complexity-rust.md`, `builtin/validators/code-hygiene/fixtures/complexity-rust.fail.rs.tmpl`, `builtin/validators/code-hygiene/fixtures/complexity-rust.pass.rs.tmpl`, `builtin/validators/code-hygiene/VALIDATOR.md`, `crates/swissarmyhammer-validators/src/review/tool_rules.rs`. Tests: 526 passed in the `swissarmyhammer-validators` lib, 7 passed in mirdan `builtin_validators`, 0 failed. `cargo fmt --check` and `cargo clippy --all-targets` clean on both crates. RED verified five ways before GREEN, and each gate isolated on the fail fixture at script level.
    - next: `/review`. The three test rosters needed no edit; the rule name and both fixture names did not change.
  timestamp: 2026-08-08T14:24:10.575098+00:00
- actor: claude-code
  id: 01kzgwr0yyzvf3rra45sfj3mj3
  text: |-
    ### review — clean
    - evidence: `review sha 68edd7698..bf5b5fc1e` (commit bf5b5fc1e, 17 files) — 0 findings, 0 confirmed, 0 refuted; 9 validators attempted, 0 failed, 0 skipped. No prior `## Review Findings` section and no open checklist items on the card.
    - next: none. Task moved to `done`. The lint choice is settled in `## Decision (settled)`; the review did not re-open it.
  timestamp: 2026-08-08T14:34:31.774541+00:00
- actor: claude-code
  id: 01kzgws43prh671nkzz0sn8525
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 5 files; Rust rule moved to the four adopted lints, false nesting claim deleted, fixtures rebuilt to trip both gates, `cognitive_complexity` rejection recorded in VALIDATOR.md
    - test: green — cargo nextest run --workspace, 13862 passed, 0 failed, 0 skipped; cargo fmt --check clean; cargo clippy --workspace --all-targets -- -D warnings clean
    - commit: bf5b5fc1e — 17 files, 1870 insertions, 67 deletions; staged by explicit path because a parallel session shares this tree
    - review: clean — 0 findings, 9 validators attempted, 0 failed, 0 skipped on `review sha 68edd7698..bf5b5fc1e`
    - result: task moved to done

    Iteration 1 ended `stuck` on the `clippy::cognitive_complexity` blocker. It was resolved by a lint survey of the full clippy 0.1.97 lint list, not by the card's fallback: `clippy::excessive_nesting` is uncontaminated by macro expansion and carries the nesting dimension of the prompt rule, so `supersedes: [cognitive-complexity, function-length]` holds as the card originally required.
  timestamp: 2026-08-08T14:35:07.766174+00:00
depends_on:
- 01KZEB9V0GBG049K0PPGWHWYT8
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffca80
title: 'complexity tool rules: Rust + Python (clippy, ruff)'
---
Add tool rules to `builtin/validators/code-hygiene` that supersede the `cognitive-complexity` and `function-length` prompt rules. Follow the missing-docs pattern and the README contract.

Rust — one rule, one clippy run, workspace scope. See `## Decision (settled)` below for the final lint set; `clippy::cognitive_complexity` is rejected and replaced by `clippy::excessive_nesting`.
- Thresholds: the run script writes a temporary `clippy.toml` and points `CLIPPY_CONF_DIR` at its directory. Never read or change the project clippy.toml. CLIPPY_CONF_DIR behavior is verified — the environment variable wins and the project file is never read.
- `supersedes: [cognitive-complexity, function-length]` — blocked by ^gwhwyt8. This holds: `excessive_nesting` gates the prompt rule's nesting dimension.

Python — two rules, files scope, ruff:
- `complexity-python`: `ruff check --isolated --no-cache --config "lint.mccabe.max-complexity=15" --select C901 --output-format json "$@"` piped through jq. Supersedes `cognitive-complexity`. Note in the rule body: C901 is cyclomatic, not Sonar cognitive; the tool gate replaces the prompt gate.
- `function-length-python`: PLR0915 with a statement threshold that approximates 250 code lines. State the chosen number and the reason in the rule body. Supersedes `function-length`.

Both languages: fail/pass fixture pairs in `fixtures/`. The fail fixture holds one function over each gate; the pass fixture holds the same shapes under the gates. Doctor shows the fixture rows.

The `complexity` tree-sitter probe stays. A language without a healthy tool keeps the probe + prompt path — that is the designed fallback. Rust does NOT need that fallback; see below.

#tool-validators

## Decision (settled)

A person chose this after a full clippy 0.1.97 lint survey. Do not re-evaluate. Implement exactly.

**Rejected: `clippy::cognitive_complexity`.** It walks the macro-expanded AST. This workspace builds `tracing` with the `log` feature, so the log bridge's branches are attributed to the caller. A flat, zero-branch function with six log calls scores 7 without the feature and 43 with it. 460 findings workspace-wide, the mass sitting just over the gate. No threshold or jq filter separates artifact from signal.

**Adopted, all measured uncontaminated:**

| Lint | Threshold | Workspace findings |
|---|---|---|
| `clippy::excessive_nesting` | `excessive-nesting-threshold = 6` | 55 |
| `clippy::too_many_lines` | `too-many-lines-threshold = 250` | 4 |
| `clippy::too_many_arguments` | `too-many-arguments-threshold = 7` | 0 |
| `clippy::type_complexity` | `type-complexity-threshold = 250` | 0 |

Keep the rule named `complexity-rust` and keep `supersedes: [cognitive-complexity, function-length]`. Nesting depth is the backbone of the Sonar cognitive metric, so `excessive_nesting` carries the supersession — but the rule body must state plainly that it gates **lexical nesting depth**, not a cognitive score.

Delete the claim now in `complexity-rust.md` that the prompt rule's nesting gate "has no clippy lint, so superseding drops it for Rust". It is measurably false.

Record the `cognitive_complexity` rejection in `VALIDATOR.md` beside the `cargo machete` / `knip` / `periphery` verdicts, with the 7 -> 43 probe number as the evidence.

### Exact run script — verified end to end, exit 0

```sh
conf="$(mktemp -d)"
trap 'rm -rf "$conf"' EXIT
printf 'excessive-nesting-threshold = 6\ntoo-many-lines-threshold = 250\ntoo-many-arguments-threshold = 7\ntype-complexity-threshold = 250\n' > "$conf/clippy.toml"
CLIPPY_CONF_DIR="$conf" cargo clippy --workspace --all-targets --message-format=json --quiet -- \
  -W clippy::excessive_nesting -W clippy::too_many_lines \
  -W clippy::too_many_arguments -W clippy::type_complexity |
  jq -c 'select(.reason == "compiler-message")
         | .message
         | select(.code.code == "clippy::excessive_nesting"
                  or .code.code == "clippy::too_many_lines"
                  or .code.code == "clippy::too_many_arguments"
                  or .code.code == "clippy::type_complexity")
         | select(.spans | length > 0)
         | {file: .spans[0].file_name, line: .spans[0].line_start, message: .message}
         | select(.file | startswith("/") | not)' |
  sort -u
```

Both pipe additions are measured as necessary:
- `sort -u` — `--all-targets` lints the lib and test targets separately, so the raw pipe emits 129 lines for 63 distinct findings.
- `select(.file | startswith("/") | not)` — `too_many_lines` fires on four generated `OUT_DIR` files that arrive as absolute paths and are unfixable build output.

Do NOT use `--force-warn`. Three of the four lints are warn-by-default and 36 legitimate inline `#[allow]`s exist in the workspace. No crate-level `#![allow]` exists for any of them.

### Rust fixtures

The Rust fail fixture must hold one function over EACH adopted gate that can be exercised in a fixture: a nesting pyramid past depth 6 and a function past 250 lines. The pass fixture holds the same shapes under both gates. Remember the item overhead — an `impl` method carries 2 levels, a free function 1 — so state in the fixture doc comment which shape is being measured and at what depth.