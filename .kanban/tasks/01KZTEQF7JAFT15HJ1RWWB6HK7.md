---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kztm98s6x9h51gj2mthw7y81
  text: |-
    Research done. All 10 delete targets exist. Line numbers verified against the current tree:

    - `shipped.rs:19` = `mod commented_code;`, `shipped.rs:28` = `mod duplication;` — the card's numbers still hold after f495f760c/0142cffbc/0de4a3936.
    - `mirdan/src/builtin_validators.rs:275-276` = the two `no-commented-code-parsed` fixture names. The duplication set ships no fixture in that roster.
    - `apps/swissarmyhammer-cli/Cargo.toml:116-118` = the `swissarmyhammer-sem` dev-dep. `tests/duplication_tool_rule.rs` is its only reader in the package, so it goes.
    - `doctor.rs:703,706,719,738,764` = `SAH_BINARY_ENV`, `SAH_BINARY_NAME`, `sah_binary()`, `is_sah_binary()`, the `command.env(...)` line. The three tests are at 814, 845 and 858, plus the `EnvVarGuard` import and the `ECHO_SAH_BIN_SCRIPT` and `BINARY_NAME_SPELLINGS` constants they alone read.

    Three files the card does not list also name the two rules, and each must change or the suite goes red or the prose goes false:

    1. `shipped/missing_docs.rs:456,466` — the `SHIPPED_RULES_THAT_READ_A_GO_FILE` roster. It falls from 27 entries to 25.
    2. `builtin/validators/code-hygiene/rules/missing-docs-go.md:81,86` — states "27 shipped rules match a `.go` file". The roster above is what holds that sentence, so the number becomes 25.
    3. `crates/swissarmyhammer-tools/src/mcp/tools/code_context/execute.rs:342,354` — doc comments naming the two rules as the readers of the two ops. The ops stay; the sentence naming a deleted rule goes.

    `rg -l '^tool:' builtin/validators` reports 27 rules today, so 25 after the two deletions.
  timestamp: 2026-08-12T09:19:03.974907+00:00
- actor: claude-code
  id: 01kztp48rggjewft6ee2rv1zds
  text: |-
    Implementation landed. Every item of the card is in the tree.

    Deleted, all 10: the two rule files, the four fixtures, the two acceptance tests under `apps/swissarmyhammer-cli/tests/`, and the two shipped-test modules. `builtin/validators/duplication/fixtures/` is now empty and gone, which matches every other prompt-only set.

    Edited, the 9 the card names plus 5 the deletion forced:

    - `shipped.rs` — `mod commented_code;` and `mod duplication;` removed.
    - `builtin/mod.rs` — `CODE_HYGIENE_COMMENTED_CODE_TOOL_RULES` and its three uses (roster chain, count, supersedes chain) removed, `COMMENTED_CODE_TOOL_RULE` and `test_commented_code_tool_rule_matches_exactly_the_extensions_the_op_covers` removed, the duplication roster count 4 → 3.
    - `mirdan/src/builtin_validators.rs` — the two fixture names out of `CODE_HYGIENE_FIXTURES`.
    - `apps/swissarmyhammer-cli/Cargo.toml` — the `swissarmyhammer-sem` dev-dep and its comment.
    - `duplication/VALIDATOR.md` — "Which tools this set uses" now states that the prompt rules decide and the `duplicates` probe supplies the machine facts. "The comparison is our own" described the deleted rule, so it is gone. The `cpd-core` and `jscpd` verdicts stay; the one sentence of the jscpd verdict that named the deleted rule as the test-code exclusion now names the reader.
    - `code-hygiene/VALIDATOR.md` — the commented-out-code section now states no tool rule. The `ruff ERA001` verdict stays, on its measured ground: it reports each line on its own and states no block-length option, so it cannot express the prompt rule's "more than 5 lines" gate, and it answers for Python alone.
    - `code-hygiene/rules/no-commented-code.md` — the fallback framing is gone; the rule always runs.
    - `builtin/validators/README.md` — the `SAH_BIN` paragraph and the plain-text-op note deleted.
    - `doctor.rs` — `SAH_BINARY_ENV`, `SAH_BINARY_NAME`, `sah_binary()`, `is_sah_binary()`, the `command.env(...)` line, the three tests, their two constants, and the now-unused `EnvVarGuard` import.

    Forced by the deletion, and not on the card:

    - `shipped/missing_docs.rs` — the two rules out of `SHIPPED_RULES_THAT_READ_A_GO_FILE`, 27 entries to 25.
    - `code-hygiene/rules/missing-docs-go.md` — "27 shipped rules match a `.go` file" to 25, twice.
    - `shipped/zero_argument.rs` — `FILES_SCOPE_RULE_COUNT` 16 → 14. Both deleted rules stated `scope: files`. The measurement in its doc is re-taken over the 14 shipped scripts: 7 write the guard on the first line, 7 write it under `set -e`.
    - `shipped/scope_roster.rs` — `SHIPPED_TOOL_RULE_COUNT` 27 → 25. `WORKSPACE_SCOPE_RULE_COUNT` stays 11.
    - `code_context/execute.rs` and `sem/.../commented_code.rs` — doc comments that named a deleted rule as the reader of an op. The ops stay; only the sentence changed.

    Discovery worth keeping: `mirdan`'s retired-validator prune works at SET granularity. It cannot remove a retired RULE FILE from a set that still ships, so a store an earlier `sah init` wrote keeps both deleted rules and doctor still reports them degraded. Card ^w6ypb8b carries that gap.

    Note on the tree: a parallel shell committed the whole working tree, this change included, into 59bd9ae5c. The diff is in HEAD rather than uncommitted.
  timestamp: 2026-08-12T09:51:17.264960+00:00
- actor: claude-code
  id: 01kztp4fx6ads65882gzcf0as1
  text: |-
    ### implement — changed
    - evidence: 10 files deleted, 14 files edited. `rg -c 'SAH_BIN' builtin/` reports 0 matches. `rg -l '^tool:' builtin/validators | wc -l` reports 25, and each of the 25 names an external tool. `cargo nextest run --workspace` reports 14136 tests run: 14136 passed, 0 skipped. `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean. `sah doctor` over the shipped set (run with `$HOME` pointed at an empty directory, so no store shadows the embed) lists no tool rule for duplication and none for commented code; every remaining tool rule reports `tool present; fixtures pass`.
    - next: /review
  timestamp: 2026-08-12T09:51:24.582907+00:00
- actor: claude-code
  id: 01kztqpgvhsg5pfh190q33a83m
  text: |
    ### review — findings
    - scope: review sha HEAD~1..HEAD (59bd9ae5c), which carries the work of ^wwb6hk7 and ^btphzkn plus a stale-assertion fix in shell/mod.rs
    - counts: 3 findings, 3 confirmed, 0 refuted, 40 attempted
    - evidence: crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:514, crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:881, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs_rust.rs:713
    - by card: ^wwb6hk7 (delete the two -parsed tool rules and SAH_BIN) drew zero findings. Finding 3 belongs to ^btphzkn (missing-docs-rust). Findings 1 and 2 belong to the shell/mod.rs stale-assertion fix.
    - next: task stays in review. The implementer acts on the three checklist items.
  timestamp: 2026-08-12T10:18:43.953581+00:00
- actor: claude-code
  id: 01kztqqwh6y5m8sq76w537xtzw
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 24 files. 10 deleted (2 rules, 4 fixtures, 2 CLI tests, 2 shipped test modules), 14 edited. All four done-when conditions measured: 0 SAH_BIN matches under builtin/, 25 tool rules all external, suite green, doctor reports no degraded rule for duplication or commented code (measured with a clean store; the installed-store prune is carded on ^w6ypb8b).
    - test: green — cargo nextest run --workspace, 14136 passed, 0 failed, 0 skipped. fmt and clippy clean. This card's VALIDATOR.md rewrite fixed the four failures carded on ^bh5ncd0; ^neb2era is fixed by a stale-assertion fix bundled into the same commit.
    - commit: 59bd9ae5c — committed by a parallel shell, which folded this card's work together with ^btphzkn and a shell/mod.rs stale-assertion fix under a message naming ^btphzkn only.
    - review: findings — crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:514, crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:881, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs_rust.rs:713. None of the three come from this card's own work.
  timestamp: 2026-08-12T10:19:28.678954+00:00
- actor: claude-code
  id: 01kztr5ps9s6kw1ajz1d3c8bpz
  text: |-
    Picked the card back up from `review` into `doing` to work the three findings of `## Review Findings (2026-08-12 05:01)`. Each premise was measured against the files on disk before any edit. All three conflict with the code. No file was changed.

    ### Finding 1 — `shell/mod.rs:514`, `deinit` is a near-duplicate of `init`

    The duplicated BODIES are already gone from the working tree. `run_lifecycle` in `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` holds the three lifecycle steps — the MCP server entry, the `Bash` permission and the `.shell/` config — and `LifecycleDirection` names the one difference. `init` and `deinit` each hold one line:

        run_lifecycle(self, LifecycleDirection::Install, scope, reporter)
        run_lifecycle(self, LifecycleDirection::Remove, scope, reporter)

    What stays is the trait. `Initializable` at `crates/swissarmyhammer-common/src/lifecycle.rs:88` declares `init` and `deinit` with the same signature, so the signature cannot change. Measured: `init` is 303 bytes and `deinit` is 304 bytes, and `diff` reports 2 changed lines — the name and the direction. `FindDuplicatesOptions::default()` in `crates/swissarmyhammer-code-context/src/ops/find_duplicates.rs` sets `min_chunk_bytes` to 100 and `min_similarity` to 0.85, so the `duplicates` probe still compares the two chunks and still reports the pair.

    To make the report stop, delete one of the two trait methods. That breaks the `Initializable` contract. This is a rule that fights a documented contract.

    ### Finding 2 — `shell/mod.rs:881`, add three guidance markers to the skills test

    The premise is false on disk. `crates/swissarmyhammer-skills/tests/shell_output_guidance.rs` already asserts all three markers inside `shell_output_guidance_states_blocking_and_no_tail`:

    - `NO_GREP_SEARCH_MARKER` = "Do not use grep to search files"
    - `RG_MARKER` = "use `rg`"
    - `NO_SHELL_EDIT_MARKER` = "Do not use shell to edit files"

    `git log -1` on that file reports d20c7f847b103c8c22bb66f8c9f35a0c62e64af5, Wed Aug 12 02:11:45 2026. `git merge-base --is-ancestor d20c7f847 HEAD~1` answers true, so the change stands BEFORE the reviewed range HEAD~1..HEAD. The parity the finding asks for is the state the repository already holds. There is no edit that can satisfy it.

    ### Finding 3 — `missing_docs_rust.rs:713`, add `reads_only_the_files_it_is_given`

    The test cannot pass. `verify_shipped_run_reads_only_its_arguments` in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` opens with an assertion:

        assert_eq!(shipped.scope, ToolScope::Files, ...)

    Its own doc states why: "The rule of the probe must state `scope: files`, because that scope is what makes the two runs different. A `workspace`-scope script takes an empty argument list for both runs, so the two halves would measure one run two times."

    `builtin/validators/code-hygiene/rules/missing-docs-rust.md:11` states `scope: workspace`. It is the only missing-docs rule that does, because `cargo clippy` lints a package, never a loose file.

    Measured over the whole shipped set: 14 rules state `scope: files`, and the shipped tests hold exactly 14 `reads_only_the_files_it_is_given` tests. The match is one to one. No `scope: workspace` rule carries the test — not `complexity-rust`, not `dead-code-rust`, not `unused-dependencies-rust`, not `magic-numbers-go`, not `function-length-go`, not `unused-code-go`. `FILES_SCOPE_RULE_COUNT` in `shipped/zero_argument.rs` is 14 and states the same count.

    Written and run to prove it. The test the finding names, with its probe and its staged-file constants, compiles and fails:

        assertion `left == right` failed: the probe for `missing-docs-rust` must name a rule that states `scope: files`, or the two runs below take the same empty argument list and the pair measures one run two times
          left: Workspace
         right: Files
        (shipped.rs:1005)

    The finding also asks the test to "verify that `cargo clippy` with no file argument does not read the entire repository". That is the opposite of what the rule states and of what an acceptance test already holds. `missing-docs-rust.md` states "The scope is `workspace` because cargo lints a package, never a loose file", and `the_shipped_rust_missing_docs_tool_rule_reports_every_workspace_member` (missing_docs_rust.rs:162) holds the run to reporting all three members of a probe workspace. Two requirements that cannot both be correct.

    The temporary test was removed. `git status` reports `missing_docs_rust.rs` clean.

    ### Tree state measured on the way

    `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean, exit 0. `cargo nextest run -p swissarmyhammer-skills -p swissarmyhammer-validators -p swissarmyhammer-tools` reports 2331 tests run: 2331 passed, 0 skipped.

    Note on the tree: the `run_lifecycle` refactor, the lowercase error messages in `shell/mod.rs`, and the quoted `exec "{real}"` in `shipped.rs` stand uncommitted in the working tree. They are not in HEAD (59bd9ae5c).
  timestamp: 2026-08-12T10:27:01.545745+00:00
- actor: claude-code
  id: 01kztr714ws7xc73r5geh3vyt1
  text: |-
    ### implement — stuck
    - evidence: 0 files changed. All three findings of `## Review Findings (2026-08-12 05:01)` are true conflicts, recorded under `## BLOCKER` in the description. Finding 1: `Initializable` (crates/swissarmyhammer-common/src/lifecycle.rs:88) imposes both `init` and `deinit` with the same signature; the bodies already delegate to `run_lifecycle`, and the 303/304-byte chunks stay over `min_chunk_bytes: 100` at 0.85 similarity. Finding 2: crates/swissarmyhammer-skills/tests/shell_output_guidance.rs already asserts all three markers, last changed d20c7f847, an ancestor of HEAD~1. Finding 3: the named test was written and run — it fails at crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1005 with `left: Workspace, right: Files`, because `missing-docs-rust` states `scope: workspace`; the temporary test was removed and the file is clean. Tree measured: `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` exit 0, `cargo nextest run -p swissarmyhammer-skills -p swissarmyhammer-validators -p swissarmyhammer-tools` 2331 passed, 0 skipped.
    - next: a person corrects the three rules. Card stays in `doing`.
  timestamp: 2026-08-12T10:27:44.924220+00:00
- actor: claude-code
  id: 01kztr7g134rm3awxm3btsvb2d
  text: 'Correction to the note on the tree. While this pass ran, a parallel shell committed the `run_lifecycle` refactor, the lowercase error messages in `shell/mod.rs` and the quoted `exec "{real}"` in `shipped.rs` as bf0cd8d34, "fix(validators,tools): close review findings on shell mod and stub script (^btphzkn)". Those changes are now in HEAD, not in the working tree. Every measurement in the comment above holds, because the file contents did not change — only where they live. `git status` outside `.kanban/` is clean.'
  timestamp: 2026-08-12T10:28:00.163894+00:00
- actor: claude-code
  id: 01kztr8m3etj0abf98tajkfvaz
  text: |-
    ### finish iteration 2 — stuck
    - implement: no-change — 0 files. All three findings are true conflicts, each measured:
      1. shell/mod.rs:514 (duplication) — the duplicated bodies are already gone; `run_lifecycle` holds the three steps and each method holds one line. What remains is the `Initializable` trait at crates/swissarmyhammer-common/src/lifecycle.rs:88, which declares both methods with the same signature. 303 and 304 bytes, 2 changed lines, against `min_chunk_bytes: 100` / `min_similarity: 0.85`. To stop the report you must delete one trait method, which breaks the trait.
      2. shell/mod.rs:881 (parity) — the premise is false on disk. crates/swissarmyhammer-skills/tests/shell_output_guidance.rs already asserts all three markers, and last changed in d20c7f847, which `git merge-base --is-ancestor d20c7f847 HEAD~1` places before the reviewed range. No edit can satisfy it.
      3. missing_docs_rust.rs:713 — the named test was written and proved RED at shipped.rs:1005: `verify_shipped_run_reads_only_its_arguments` asserts `ToolScope::Files`, and `missing-docs-rust` states `scope: workspace` because cargo clippy lints a package, never a loose file. The shipped set holds 14 `scope: files` rules and exactly 14 such tests, one to one, and no `scope: workspace` rule carries it. The temporary test was reverted.
    - test: green — cargo nextest run -p swissarmyhammer-skills -p swissarmyhammer-validators -p swissarmyhammer-tools, 2331 passed, 0 skipped. fmt and clippy clean.
    - review: not re-run. The card is STUCK on the three conflicts above and stays in `review`.

    This card's own work drew ZERO findings and is complete and green. All three blockers came from other cards' work that a parallel shell folded into commit 59bd9ae5c. A person must correct the three rules before this card can close.
  timestamp: 2026-08-12T10:28:37.102343+00:00
- actor: claude-code
  id: 01kztwqe7dzjydsvksahpwjvvk
  text: |-
    ### The three blocking rules are corrected — 4e41d04ab

    The user made the call: correct the rule prose in `builtin/`.

    **`builtin/validators/duplication/rules/duplication.md`** — finding 1. The dispatch-forced delegation shim carve-out ALREADY covered `init`/`deinit` word for word; the probe's measurement overrode it. Added: the body decides, never the measurement. A trait- or interface-required pair carries its signature and its doc comment into the token count and the similarity score, the declaration forces those bytes, and no edit removes them while the contract stands. The 303/304-byte `init`/`deinit` pair is now stated in the rule as the SHAPE of the carve-out, not an exception to it.

    **`builtin/validators/completeness/rules/invariant-propagation.md`** — findings 2 and 3, which came from one rule and one defect. A `clone-siblings` row is a candidate; two gates now stand before a finding:
    1. Read the sibling and confirm the treatment is ABSENT, quoting the absent line. Check the age against the range under review.
    2. Confirm the sibling ADMITS the treatment, reading what it declares about itself — front matter, declared scope, the assertion its shared helper opens with.

    Two matching exceptions were added, the second naming the `scope: workspace` against `scope: files` case directly.

    **Verification.** `cargo nextest run -p swissarmyhammer-validators` green with the edits in place. 4 `mirdan` clone tests failed in the batch run; they were proved network flakes, passing both with the edits stashed and with the edits in place when run isolated (60s timeouts against 2.4s passes).

    **This card can now be re-reviewed.** Its own work drew zero findings and is green. A fresh `/review wwb6hk7` over 59bd9ae5c should return clean and move it to done.

    Follow-up carded: ^q2cncse — a recorded finding must name the validator set and the rule that produced it. Attributing these three took four searches and one attribution was never proved, only inferred.
  timestamp: 2026-08-12T11:46:36.909326+00:00
position_column: review
position_ordinal: '8480'
title: Delete the two -parsed tool rules that shell out to sah itself
---
`duplication-parsed` and `no-commented-code-parsed` are the only two tool rules
whose tool IS sah. Each one spawns bash, which runs the `sah` binary again, to
reach a function already linked into the calling process
(`find_duplicates_in`, called in-process at
`crates/swissarmyhammer-validators/src/review/probes.rs:838`).

Remove both rules. The prompt rules they supersede run again:
`duplication`, `rust` and `swift` for duplication; `no-commented-code` for
commented code. The `duplicates` probe stays and keeps feeding them.

Removing them deletes the whole self-shell contract: the `SAH_BIN` variable,
its three-step resolution, and the doctor presence check for a tool that is
statically linked into the process that asks.

## Delete these files

- `builtin/validators/duplication/rules/duplication-parsed.md`
- `builtin/validators/duplication/fixtures/duplication-parsed.fail.rs.tmpl`
- `builtin/validators/duplication/fixtures/duplication-parsed.pass.rs.tmpl`
- `builtin/validators/code-hygiene/rules/no-commented-code-parsed.md`
- `builtin/validators/code-hygiene/fixtures/no-commented-code-parsed.fail.rs.tmpl`
- `builtin/validators/code-hygiene/fixtures/no-commented-code-parsed.pass.rs.tmpl`
- `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs`
- `apps/swissarmyhammer-cli/tests/commented_code_tool_rule.rs`
- `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/duplication.rs`
- `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/commented_code.rs`

## Edit these files

- `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:19,28`
  — remove `mod commented_code;` and `mod duplication;`.
- `crates/swissarmyhammer-validators/src/builtin/mod.rs` — the rule-roster
  tests count the rules of each set. Correct both counts and both rosters.
- `crates/mirdan/src/builtin_validators.rs:275-276` — remove the two fixture
  names from the install roster.
- `apps/swissarmyhammer-cli/Cargo.toml:116-118` — the `swissarmyhammer-sem`
  dev-dependency exists only for the `duplication-parsed` acceptance test.
  Remove it if no other test in the package reads it.
- `builtin/validators/duplication/VALIDATOR.md` — the "Which tools this set
  uses" section names the tool rule as the decider. State that the prompt
  rules decide and the `duplicates` probe supplies the machine facts. Keep the
  `cpd-core` and `jscpd` verdicts: they are still true.
- `builtin/validators/code-hygiene/VALIDATOR.md:218` — remove the tool-rule
  entry for `sah tool code_context commented_code find`.
- `builtin/validators/code-hygiene/rules/no-commented-code.md:12` — the rule
  states it runs only when the tool rule cannot. It always runs now.
- `builtin/validators/README.md:200-211` — delete the `SAH_BIN` paragraph and
  the note that an op a tool rule calls returns plain text. No rule calls an
  op any more.
- `crates/swissarmyhammer-validators/src/doctor.rs:698-742,764` — delete
  `SAH_BINARY_ENV`, `SAH_BINARY_NAME`, `sah_binary()`, `is_sah_binary()`, the
  `command.env(...)` line in `run_shell`, and the three tests that read them.

## Keep

The `code_context duplication find` and `code_context commented_code find` ops
stay. They are user-facing MCP and CLI ops with their own consumers, and
`find_duplicates_in` still serves the `duplicates` probe.

## Done when

- No file under `builtin/` names `SAH_BIN`.
- `rg -l '^tool:' builtin/validators` lists 25 rules, all of them external
  tools.
- `cargo nextest run --workspace` is green.
- `sah doctor` reports no degraded tool rule for duplication or commented code. #tool-validators #objectivity

## Review Findings (2026-08-12 05:01)

- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:514` — fn `deinit` is a near-duplicate of `init` at crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:468 (208 tokens, 95% alike).
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:881` — The test was updated to verify new guidance markers ("Do not use grep to search files", "use `rg`", "Do not use shell to edit files") in the shell tool description. Per the comment at lines 865-870, this guidance is duplicated in both the tool description and the shell skill description by design. The parallel test in swissarmyhammer-skills that verifies the skill description was left unchanged, breaking parity between the two tests that should enforce the same invariant. Update shell_output_guidance_states_blocking_and_no_tail in swissarmyhammer-skills to also assert the presence of the three new guidance markers, maintaining parity with this updated test.
- [ ] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs_rust.rs:713` — Every language's missing-docs tool tests include a 'reads_only_the_files_it_is_given' test that verifies the tool respects file arguments and does not walk the repository by default — but Rust's tests omit this pattern entirely. Add `the_shipped_rust_missing_docs_tool_rule_reads_only_the_files_it_is_given()` test (with its supporting probe and staged files constants) to verify that `cargo clippy` with no file argument does not read the entire repository, matching the pattern from all other languages.

## BLOCKER — the three findings above conflict with the code (2026-08-12)

Each premise was measured against the files on disk. No finding names work
this repository can do. Do NOT check the three boxes. A person must correct
the rules, then start the work again. The measurements are in the comment
thread.

### Finding 1 — a rule that fights a documented contract

The duplicated bodies are gone. `run_lifecycle` in
`crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` holds the three
lifecycle steps, and `init` and `deinit` each hold one line that differs only
in `LifecycleDirection::Install` against `LifecycleDirection::Remove`.

What stays is the trait. `Initializable` at
`crates/swissarmyhammer-common/src/lifecycle.rs:88` declares `init` and
`deinit` with the same signature. Measured: 303 and 304 bytes, 2 changed
lines. `FindDuplicatesOptions::default()` sets `min_chunk_bytes` to 100 and
`min_similarity` to 0.85, so the `duplicates` probe still reports the pair.
To stop the report, delete one of the two trait methods, which breaks the
trait.

### Finding 2 — the asked-for state already holds

`crates/swissarmyhammer-skills/tests/shell_output_guidance.rs` already asserts
all three markers inside `shell_output_guidance_states_blocking_and_no_tail`.
The file last changed in d20c7f847, which `git merge-base --is-ancestor`
reports as an ancestor of HEAD~1 — before the reviewed range. No edit can
satisfy the finding.

### Finding 3 — two requirements that cannot both be correct

`verify_shipped_run_reads_only_its_arguments` asserts
`shipped.scope == ToolScope::Files`. `missing-docs-rust` states
`scope: workspace`, because `cargo clippy` lints a package, never a loose
file. The test the finding names was written and run: it fails at
`shipped.rs:1005` with `left: Workspace, right: Files`, then it was removed.

The shipped set holds 14 `scope: files` rules and exactly 14
`reads_only_the_files_it_is_given` tests — one to one. No `scope: workspace`
rule carries the test.

The finding also asks the test to hold `cargo clippy` to NOT reading the whole
repository. `missing-docs-rust.md` states the opposite, and
`the_shipped_rust_missing_docs_tool_rule_reports_every_workspace_member` holds
the run to all three members of a probe workspace.
