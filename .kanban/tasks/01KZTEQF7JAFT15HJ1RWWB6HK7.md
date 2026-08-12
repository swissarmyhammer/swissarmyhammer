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
position_column: doing
position_ordinal: '8280'
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