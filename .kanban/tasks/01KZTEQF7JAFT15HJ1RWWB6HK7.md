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
position_column: doing
position_ordinal: '8380'
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
