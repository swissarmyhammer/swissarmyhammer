---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzbbm09trf98cwmpdjz143v3
  text: |-
    Check-in from the planning session (2026-08-06):

    1. Your foundation is verified. ^ygt2rre landed as 3d93fa129 and matches the contract: one match code path (`matches_criteria`), AND semantics, fail closed on unresolved context. Build on `detected_project_type_keys` in review/scope.rs.

    2. Heads-up on your acceptance item "Expose the tool block on list/dump validators": those ops resolve NO project types (fail closed — see the ^ygt2rre research note). A tool rule keyed on `match.project_types` will not appear in a `dump validators` result until a workspace root is threaded in. That wiring is now its own card, ^3hwy2pd — do NOT scope-creep it into this task. For this task, prove exposure with a tool rule that has no `project_types` key, or assert on the loaded validator set directly.

    3. Contract reminder from the final design round (re-read the committed README if your copy predates 44259b9ba): the tool block is exactly `scope`, `run`, `doctor`, `install`. No output/format/jq/regex/filter keys, no exit.findings. The `run` script's stdout is the whole findings contract.
  timestamp: 2026-08-06T10:59:02.074963+00:00
- actor: claude-code
  id: 01kzbbskt636n6jttcyxyeegdq
  text: |-
    ### research
    - Contract read: builtin/validators/README.md — tool rule = rule file + `tool` block, `supersedes`, rule-level `match` narrows set match, stdout contract (path:line: message | jq -c JSON object), empty stdout = clean, exit 0 = judged.
    - The card path `swissarmyhammer-validators/src/validators/types.rs` is `crates/swissarmyhammer-validators/src/validators/types.rs` in this repo.
    - Key code points:
      - `types.rs`: `ValidatorMatch` (tools/files/project_types, AND across keys, OR in a key), shared `matches_criteria` used by both `Validator::matches` and `RuleSet::matches`. `Rule` carries name/description/body/timeout. `RuleFrontmatter` same.
      - `parser.rs`: `parse_rule(content, path)` — no expander today, so rule-level `@file_groups` needs a signature change to accept `Option<&YamlExpander<C>>`. `parse_ruleset_directory` fails the WHOLE set on one bad rule (`parse_rule(...)?`) — must change to record the failure and keep the set.
      - `loader.rs`: builtin/user/project all flow through `load_rulesets_directory` → `parse_ruleset_directory`. `LoadFailure` exists for set-level failures.
      - `swissarmyhammer-tools/src/mcp/tools/review/validators.rs`: `RuleDetail {name, body}` backs `list validators` (rules:true) and `get validator` — extend with `tool` + `supersedes`.
      - `review/types.rs`: `Finding {file, line, validator, rule, claim, evidence, suggestion}` — the stdout parser maps message→claim, raw line→evidence.
    - Plan: ToolSpec/ToolScope/ToolDoctor/ToolInstall types with deny_unknown_fields (rejects output/format/jq keys with one clear serde error); `Rule::matches(ruleset, ctx)` = `ruleset.matches` AND rule `matches_criteria` (same code path, narrow-only); `RuleSet.rule_failures: Vec<RuleLoadFailure>` so a malformed rule is reported by `check validators` without dropping the set; new `review/tool_output.rs` stdout parser.
  timestamp: 2026-08-06T11:02:05.894389+00:00
- actor: claude-code
  id: 01kzbcq30zm3d3n0gdyeemq22s
  text: |-
    ### implement — changed
    - evidence: 10 files.
      - `crates/swissarmyhammer-validators/src/validators/types.rs` — new `ToolScope`, `ToolDoctor`, `ToolInstall`, `ToolSpec` (deny_unknown_fields rejects output/format/jq/exit keys with one clear serde error); `RuleFrontmatter` + `Rule` gain optional `match` (narrow-only), `supersedes`, `tool`; `Rule::matches(ruleset, ctx)` = set match AND rule `matches_criteria` — the same shared matcher path the sets use; `RuleLoadFailure`; `RuleSet.rule_failures`. Tests: narrow, never-widen, inherit, is_tool_rule.
      - `crates/swissarmyhammer-validators/src/validators/parser.rs` — `parse_rule` now takes the `@` expander (rule-level `match` expands `@file_groups` like the manifest); a malformed rule is skipped and recorded in `rule_failures` instead of dropping the whole set. Tests: full README tool-rule shape, workspace scope, forbidden `format:` key rejected naming the file, `@file_groups` expansion, malformed-tool-block-keeps-the-set.
      - `crates/swissarmyhammer-validators/src/validators/loader.rs` — test: a tool rule loads through the existing user/project layer precedence with its tool block and supersedes intact.
      - `crates/swissarmyhammer-validators/src/review/tool_output.rs` (new) — `parse_tool_stdout`: both contract shapes (`path:line: message` and jq `{file,line,message}`) into `Finding` fields (message→claim, raw line→evidence), empty stdout = clean, contract-breaking line = error naming the line. 8 tests.
      - `crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs` — `RuleDetail` exposes `tool` + `supersedes` on `list validators` (rules:true) and `get validator`; `check validators` lints rule-level globs and surfaces `rule_failures`.
      - `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs` — registered-tool test: get/list expose the tool block; prompt rule rows carry neither key.
      - mod.rs re-exports; test_support/fleet test literals updated for the new fields.
    - commands: `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-tools` → 1921 passed, 0 failed. `cargo fmt` clean. `cargo clippy --all-targets -- -D warnings` clean. `cargo check --workspace --all-targets` no errors.
    - note: execution of the `run` script stays out of the review engine per the card — parsing functions only (^cbnfe97 owns execution).
    - next: test
  timestamp: 2026-08-06T11:18:11.743544+00:00
depends_on:
- 01KZ9497ZJ3WRCAG1Z6YGT2RRE
position_column: doing
position_ordinal: '8280'
title: 'Tool-rule schema: same rule metadata, added tool block'
---
Add the `tool` rule kind. A tool rule is a normal rule file in `rules/` with the same core metadata, plus a `tool` block in frontmatter. There is NO separate runner file, directory, schema, or matcher.

The contract is `builtin/validators/README.md`. Implement that spec exactly.

Work:
- Extend the existing rule frontmatter types in `swissarmyhammer-validators/src/validators/types.rs`. Add optional `tool` and optional `supersedes`.
- The `tool` block is small: `scope` (files|workspace), `run` (a shell script — the pipeline IS the mapping, like skills embed shell), `doctor` (check_command, check_version_command), `install.commands`. There is NO output/format/jq/regex/filter configuration and NO exit.findings key.
- Stdout contract of `run`: one finding per line, either `path:line: message` or a `jq -c` style JSON object `{file, line, message}`. Empty stdout = clean. Exit 0 = judged. Nonzero exit = tool broke.
- Matching REUSES `ValidatorMatch` (`match:` block, `files:` globs, `@file_groups` expansion). Do not build a second matcher. The `project_types` match key is task ^ygt2rre, not this task.
- Allow a rule-level `match` that NARROWS the set's match (intersection). Today rules inherit and cannot override — change this to narrow-only. A rule never matches a file its set does not match.
- Existing prompt rules parse unchanged. A rule with a `tool` block is a tool rule.
- Expose the tool block and supersedes on `list validators` / `get validator` output.

Acceptance:
- A tool rule in any layer (builtin/user/project) loads by the existing precedence.
- A rule-level match intersects the set match, proven by test against the SAME matcher code path the sets use.
- Both stdout line shapes parse into Finding fields, proven by test.
- A malformed tool block reports one clear error and does not break the set.

#tool-validators