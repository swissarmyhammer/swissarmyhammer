---
assignees:
- claude-code
position_column: todo
position_ordinal: ffd680
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