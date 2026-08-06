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
- actor: claude-code
  id: 01kzbesb1et64s3t2v04772j1k
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (6e9c1d8f4); engine counts: 22 findings, 22 confirmed, 19 refuted. 12 findings removed by the skill rule for pre-existing test code. 10 findings recorded: tests.rs:354, validators.rs:374, validators.rs:515, validators.rs:529, loader.rs:681, parser.rs:277, parser.rs:407, parser.rs:465, types.rs:111, types.rs:117
    - next: implement the 10 open checklist items in the 2026-08-06 06:23 section, then run /review again
  timestamp: 2026-08-06T11:54:22.638371+00:00
- actor: claude-code
  id: 01kzbet3bqrpagz7mjtdvhb9hz
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 10 files (validators: types.rs, parser.rs, loader.rs, mod.rs, review/tool_output.rs new, review/mod.rs, test_support.rs, fleet/tests.rs; tools: review/validators.rs, review/tests.rs)
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3138 passed, 0 failed; fmt clean; clippy -D warnings clean
    - commit: 6e9c1d8f4
    - review: findings — 10 open: tools/review/tests.rs:354, tools/review/validators.rs:374,515,529, validators/loader.rs:681, validators/parser.rs:277,407,465, validators/types.rs:111,117
  timestamp: 2026-08-06T11:54:47.543293+00:00
- actor: claude-code
  id: 01kzbfj36x56j1j0pvxc2pt8nk
  text: |-
    ### implement — changed
    - evidence: 6 files changed; all 10 review findings from the 2026-08-06 06:23 section are done and checked.
      - `crates/swissarmyhammer-validators/src/validators/types.rs` — `with_changed_files` and `with_project_types` now accept `impl IntoIterator<Item = String>` (types.rs:111, types.rs:117).
      - `crates/swissarmyhammer-validators/src/review/scope.rs` — the one caller now passes `project_types.iter().cloned()` and does not allocate a Vec.
      - `crates/swissarmyhammer-validators/src/validators/parser.rs` — new shared helpers `parse_yaml_frontmatter`, `expand_yaml_includes`, and `parse_frontmatter_block`; all three parse paths (`parse_validator_internal`, `parse_ruleset_manifest`, `parse_rule`) use them, so no YAML-parse or include-expansion copy remains (parser.rs:277, parser.rs:407). `parse_ruleset_directory` decomposed into `require_ruleset_layout`, `is_rule_file`, and `parse_rule_in_set` — below the complexity gate (parser.rs:465).
      - `crates/swissarmyhammer-validators/src/review/test_support.rs` — new shared fixture `write_tool_rule_ruleset(base, name, glob, run)`; both test files import it, no local copy remains (loader.rs:681).
      - `crates/swissarmyhammer-validators/src/validators/loader.rs` — local `write_tool_ruleset` deleted; test uses the shared fixture.
      - `crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs` — `render_rules_markdown` now renders `Supersedes: <rule>` for a rule that declares one (validators.rs:374); shared `validate_glob_patterns` helper replaces the two glob-validation loops in `lint_ruleset` (validators.rs:515, validators.rs:529).
      - `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs` — new TDD test `dump_validators_renders_tool_rule_supersedes` (RED first: markdown had no Supersedes line; GREEN after the render change) (tests.rs:354); local fixture copy deleted, shared fixture imported, `TOOL_RULE_RUN` constant added.
    - commands: `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-tools` → 1922 passed, 0 failed. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` → 3139 passed, 0 failed. `cargo fmt` clean. `cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings` clean.
    - next: test
  timestamp: 2026-08-06T12:07:53.821004+00:00
- actor: claude-code
  id: 01kzbfpxg5qf8gj029w8gcnn4r
  text: |-
    ### test — green
    - evidence: `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3139 passed, 0 failed, 0 skipped (2 slow, 2 leaky, all pass); `cargo fmt --check` clean; `cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings` clean; no `#[ignore]` tests found in touched paths.
    - next: ready for review/commit.
  timestamp: 2026-08-06T12:10:31.813141+00:00
- actor: claude-code
  id: 01kzbhnjmmwvxcx3jbap0pnyea
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (d3f1e0a15). Engine: 32 confirmed. The written test-code exception removed 11. 21 findings recorded in `## Review Findings (2026-08-06 07:11)`: validators.rs:521, test_support.rs:346, types.rs:30, 93, 99, 105, 111, 117, 173, 224, 466, 473, 504, 509, 531, 589, 608, 759, 767, 872, 877. All 10 prior items (2026-08-06 06:23) are checked.
    - next: resolve the 21 open findings, then run /review ^q4909tf again.
  timestamp: 2026-08-06T12:44:45.076903+00:00
- actor: claude-code
  id: 01kzbhp8ky2kvn75tp9z3atftp
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — worked all 10 findings from 2026-08-06 06:23 checklist; 6 files
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3139 passed, 0 failed; fmt clean; clippy -D warnings clean
    - commit: d3f1e0a15
    - review: findings — 21 open: tools/review/validators.rs:521; validators/review/test_support.rs:346; validators/types.rs:30,93,99,105,111,117,173,224,466,473,504,509,531,589,608,759,767,872,877 (derives, duplicated builders/defaulting/delegation, glob param type, panicking helper)
  timestamp: 2026-08-06T12:45:07.582373+00:00
- actor: claude-code
  id: 01kzbj8wve78zbvbcm2h06n827
  text: |-
    ### implement — changed
    - evidence: 6 files changed; all 21 findings in the 2026-08-06 07:11 section are done and checked.
      - `crates/swissarmyhammer-validators/src/validators/types.rs` — the cause of each duplication finding is removed for the whole file, not only the cited lines: `builder_into!`/`builder_iter!` macros back all five MatchContext builders (types.rs:93, 99, 105, 111, 117); `result_method!` macro backs `ValidatorResult::pass`/`fail` (types.rs:466, 473); new public trait `HasValidatorResult` with default `passed()`/`message()` replaces the byte-identical delegation on `ExecutedValidator` and `RuleResult` (types.rs:504, 509, 872, 877); shared helpers `default_name_from_path` and `default_description` back all three `apply_defaults` implementations (types.rs:224, 589, 759, 767); `PartialEq, Eq` derives added to the cited structs (ValidatorMatch :30, ValidatorFrontmatter :173, RuleSetManifest :531, Rule :608) and, to remove the cause file-wide, also to MatchContext, Validator, ValidatorResult, ExecutedValidator, RuleSetMetadata, RuleFrontmatter, RuleSet, RuleResult, and ExecutedRuleSet.
      - `crates/swissarmyhammer-validators/src/validators/mod.rs` — re-exports `HasValidatorResult`.
      - `crates/swissarmyhammer-validators/src/review/test_support.rs` — `write_tool_rule_ruleset` returns `Result<(), std::io::Error>` and propagates I/O failures with `?` (test_support.rs:346).
      - `crates/swissarmyhammer-validators/src/validators/loader.rs` — the two fixture calls now `.expect(...)` the returned Result.
      - `crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs` — `validate_glob_patterns` takes `globs: &[impl AsRef<str>]` with the prescribed loop body (validators.rs:521).
      - `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs` — the two fixture calls now `.expect(...)` the returned Result.
    - commands: `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` → 3139 passed, 0 failed, 0 skipped. `cargo fmt --check` clean. `cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings` clean.
    - next: test
  timestamp: 2026-08-06T12:55:18.126121+00:00
- actor: claude-code
  id: 01kzbk9h802mkg0sq2pc1prng5
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (commit 1e04d5600). The engine reported 19 findings. The written test-refactor exception removed all 19 — each finding points at test code that this commit did not add (tests.rs tokio attributes and concurrency values, test_support.rs seed_symbol fixture, types.rs test-module timeout values at old lines 1415/1531/1739/1759 shifted +58). Zero findings recorded. All prior checklist items in the 06:23 and 07:11 sections are checked.
    - next: none. Task moved to done.
  timestamp: 2026-08-06T13:13:07.584437+00:00
- actor: claude-code
  id: 01kzbkada51am41xfsvn80zt50
  text: |-
    ### finish iteration 3 — clean
    - implement: changed — worked all 21 findings from 2026-08-06 07:11 checklist; builder_into!/builder_iter!/result_method! macros, HasValidatorResult trait, shared defaulting helpers, PartialEq/Eq derives, glob param &[impl AsRef<str>], Result-returning write_tool_rule_ruleset
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3139 passed, 0 failed; fmt clean; clippy -D warnings clean
    - commit: 1e04d5600
    - review: clean — 0 findings (19 engine candidates all dropped by the written existing-test exception); task moved to done
  timestamp: 2026-08-06T13:13:36.325729+00:00
depends_on:
- 01KZ9497ZJ3WRCAG1Z6YGT2RRE
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffb080
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

## Review Findings (2026-08-06 06:23)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:354` — The new test `get_validator_exposes_tool_block_and_supersedes` verifies that `get validator` (line 376) and `list validators` with `rules: true` (line 401) both expose the new `tool` and `supersedes` fields on tool rules. However, the test does not exercise `dump validators` with tool rules. If tool rules and `supersedes` are user-visible features, all operations that expose rules should be tested to ensure consistent behavior across the API. Add a test case that calls `dump validators` on a path matched by the tool-rule validator and verifies the markdown output contains the `supersedes` information (and any other tool-related details that should be user-visible). Ensure all operations that expose rules work consistently with tool rules.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs:374` — The `rule_details` function now populates `supersedes` (line 179) and `tool` (line 180) fields for all rules, making them available for all callers. However, `render_rules_markdown` (which calls `rule_details` at line 374) only renders `name` and `body` (line 375), discarding the new fields. Users calling `dump validators` will not see tool/supersedes information even though that information is centrally populated and exposed via the structured API (`get validator`, `list validators`). At minimum, `supersedes` should be included in the markdown to show rule relationships consistently with the API. Update `render_rules_markdown` to include `supersedes` in the markdown output (e.g., add a line like `if let Some(s) = &rule.supersedes { doc.push_str(&format!("Supersedes: {}\n\n", s)); }` after line 375). The `tool` block may be too complex for markdown, but `supersedes` is a simple string that users need to understand rule relationships.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs:515` — Glob validation logic repeats at line 529 — extract to a shared helper parameterized by error message. Extract a helper `fn validate_glob_patterns(globs: &[&str], path: &str, context: &str, errors: &mut Vec<ValidatorProblem>)` that takes the error message context as a parameter. Call it once for ruleset globs and once per rule.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs:529` — Glob validation logic duplicates at line 515 — extract to a shared helper parameterized by error message. Extract a helper `fn validate_glob_patterns(globs: &[&str], path: &str, context: &str, errors: &mut Vec<ValidatorProblem>)` that takes the error message context as a parameter. Call it once for ruleset globs and once per rule.
- [x] `crates/swissarmyhammer-validators/src/validators/loader.rs:681` — New test helper `write_tool_ruleset()` duplicates existing test setup code — at 0.97 similarity with an identical function in another test file, it should be extracted to shared test utilities or reused rather than redefined. Extract to shared test support module (e.g., `crates/swissarmyhammer-validators/src/review/test_support.rs`) and import from both test files, or import and reuse the existing function.
- [x] `crates/swissarmyhammer-validators/src/validators/parser.rs:277` — Verbatim copy of YAML parsing code at line 162: identical error handling, message, and exception structure, differing only in the validator identifier construction. Two blocks that differ only by a value are one function with an argument. Extract a helper function `fn parse_yaml_frontmatter(rendered: &str, validator_id: &str) -> Result<serde_yaml_ng::Value, AvpError>` that takes the validator identifier as a parameter, and call it from both `parse_validator_internal` (line 162) and `parse_ruleset_manifest` (line 277).
- [x] `crates/swissarmyhammer-validators/src/validators/parser.rs:407` — Verbatim copy of YAML include expansion code at line 169: identical conditional, expander call, and error structure, differing only in the validator identifier. This is the third occurrence of the same expansion pattern. Use the extracted helper function `expand_yaml_includes()` in `parse_rule` (line 407), eliminating the third copy and ensuring all three parsing functions expand includes identically.
- [x] `crates/swissarmyhammer-validators/src/validators/parser.rs:465` — Function `parse_ruleset_directory` has cognitive complexity of 15, meeting the gate threshold. Complexity arises from sequential error checks (5 distinct error conditions causing early returns), a loop over directory entries with conditional branching (skip non-directories, skip partials, check for manifest, parse rule), and nested match/error handling within the loop. Extract rule parsing and validation logic into a separate helper function `parse_rule_in_set` to reduce the main loop's branching factor. The five error checks at the start could also be consolidated or extracted into a validation function.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:111` — Accept generic iterables, not concrete Vec types — enables callers to pass arrays, iterators, or other collection types without requiring allocation or owned data. Change signature to accept impl IntoIterator: `pub fn with_changed_files<I: IntoIterator<Item = String>>(mut self, files: I) -> Self { self.changed_files = Some(files.into_iter().collect()); self }`.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:117` — Accept generic iterables, not concrete Vec types — enables callers to pass arrays, iterators, or other collection types without requiring allocation or owned data. Change signature to accept impl IntoIterator: `pub fn with_project_types<I: IntoIterator<Item = String>>(mut self, project_types: I) -> Self { self.project_types = Some(project_types.into_iter().collect()); self }`.

Note: The review engine reported 22 findings. The review skill has a written exception: do not record findings that ask for changes to test code that existed before this commit. This rule removed 12 findings: `tests.rs:1402`, `test_support.rs:141`, `test_support.rs:263`, `test_support.rs:282`, `test_support.rs:317`, `test_support.rs:572`, `test_support.rs:954`, `test_support.rs:1362`, `types.rs:947`, `types.rs:1415`, `types.rs:1531`, `types.rs:1739`. All of these point at test code that this commit did not add.

## Review Findings (2026-08-06 07:11)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs:521` — Parameter `globs: &[String]` accepts only concrete `String` slices when the function only needs `&str` from each element (passed to `glob::Pattern::new()`). Accept generics instead: `&[impl AsRef<str>]` improves caller flexibility and follows established Rust API design principles. Change parameter to `globs: &[impl AsRef<str>]` and update the loop body: `for glob_item in globs { let glob = glob_item.as_ref(); if glob::Pattern::new(glob).is_err() { ... } }`.
- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs:346` — Panics on expected failure mode: `fs::write()` can fail with permission denied, disk full, or invalid path — these are not bugs but legitimate I/O failures that should be propagated, not panicked. Return `Result<(), std::io::Error>` from `write_tool_rule_ruleset()` and propagate the error with `?` instead of `.unwrap()`.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:30` — ValidatorMatch is a public configuration struct with comparable fields (Vec<String>) but does not implement PartialEq and Eq. Downstream code cannot compare instances or use it in tests with `==` without implementing these traits themselves, violating orphan rules. Add `PartialEq, Eq` to the derive macro at line 30: `#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]`.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:93` — Builder method `with_tool` is verbatim copy-pasted as `with_file` and `with_event_context`, differing only by method name, parameter name, and field name. This creates maintenance burden: logic that could drift out of sync across the three copies. Extract using a macro: `macro_rules! builder_into { ($name:ident, $field:ident) => { pub fn $name(mut self, value: impl Into<String>) -> Self { self.$field = Some(value.into()); self } } }` then invoke as `builder_into!(with_tool, tool_name); builder_into!(with_file, file_path); builder_into!(with_event_context, event_context);`.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:99` — Builder method `with_file` is a verbatim copy of `with_tool` and `with_event_context`, differing only by method name, parameter name, and field name. Use the macro extraction described in the `with_tool` finding (line 93).
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:105` — Builder method `with_event_context` is a verbatim copy of `with_tool` and `with_file`, differing only by method name, parameter name, and field name. Use the macro extraction described in the `with_tool` finding (line 93).
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:111` — Builder method `with_changed_files` is verbatim copy-pasted as `with_project_types`, differing only by method name, parameter name, and field name. This creates maintenance burden: logic that could drift out of sync across the two copies. Extract using a macro: `macro_rules! builder_iter { ($name:ident, $field:ident) => { pub fn $name<I: IntoIterator<Item = String>>(mut self, value: I) -> Self { self.$field = Some(value.into_iter().collect()); self } } }` then invoke as `builder_iter!(with_changed_files, changed_files); builder_iter!(with_project_types, project_types);`.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:117` — Builder method `with_project_types` is a verbatim copy of `with_changed_files`, differing only by method name, parameter name, and field name. Use the macro extraction described in the `with_changed_files` finding (line 111).
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:173` — ValidatorFrontmatter is a public data struct with comparable fields (String, Option fields) but does not implement PartialEq and Eq. Downstream code cannot compare frontmatter instances or assert equality in tests, violating orphan rules. Parallel to ValidatorMatch. Add `PartialEq, Eq` to the derive macro at line 173: `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]`.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:224` — Name defaulting logic in ValidatorFrontmatter::apply_defaults is byte-identical copy-pasted in RuleFrontmatter::apply_defaults. Both extract name from file stem and use identical chain; should be extracted to shared helper to prevent drift. Extract to helper function: `fn default_name_from_path(name: &mut String, path: &Path) { if name.is_empty() { *name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unnamed").to_string(); } }` and call from both apply_defaults implementations.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:466` — Factory method ValidatorResult::pass is structurally identical to ValidatorResult::fail, differing only by enum variant name (Passed vs Failed) and method name. Both methods contain identical logic for creating a result with a message. Extract using a macro: `macro_rules! result_method { ($name:ident, $variant:ident) => { pub fn $name(message: impl Into<String>) -> Self { Self::$variant { message: message.into() } } } }` then invoke as `result_method!(pass, Passed); result_method!(fail, Failed);`.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:473` — Factory method ValidatorResult::fail is structurally identical to ValidatorResult::pass, differing only by enum variant name (Failed vs Passed) and method name. Use the macro extraction described in the line 466 finding.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:504` — Method ExecutedValidator::passed is byte-identical delegation to RuleResult::passed, differing only in container type. Both methods contain identical code delegating to self.result.passed(). Extract to a shared trait: `trait HasValidatorResult { fn result(&self) -> &ValidatorResult; }` with default methods `passed()` and `message()`, then implement the trait on both ExecutedValidator and RuleResult (requiring only one method to access the result field).
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:509` — Method ExecutedValidator::message is byte-identical delegation to RuleResult::message, differing only in container type. Use the shared trait approach described in the line 504 finding.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:531` — RuleSetManifest is a public configuration struct with comparable fields (String, Option, Vec fields) but does not implement PartialEq and Eq. Downstream code cannot compare manifests or assert equality in caching or testing scenarios, violating orphan rules. Add `PartialEq, Eq` to the derive macro at line 531: `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]`.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:589` — Description defaulting in RuleSetManifest::apply_defaults is a copy-paste of the pattern in ValidatorFrontmatter and RuleFrontmatter, differing only in the template string literal. Use the parameterized helper function described in the line 233 finding.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:608` — Rule is a public data struct with comparable fields (String name/description/body, Option fields) but does not implement PartialEq and Eq. Downstream code cannot compare rule instances or assert equality in tests, violating orphan rules. Structurally parallel to ValidatorFrontmatter and RuleFrontmatter. Add `PartialEq, Eq` to the derive macro at line 608: `#[derive(Debug, Clone, Default, PartialEq, Eq)]`.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:759` — Name defaulting logic in RuleFrontmatter::apply_defaults is byte-identical copy-pasted from ValidatorFrontmatter::apply_defaults. Both extract name from file stem using identical code; should be extracted to shared helper to prevent drift. Use the helper function described in the line 224 finding.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:767` — Description defaulting in RuleFrontmatter::apply_defaults is a copy-paste of the pattern in ValidatorFrontmatter and RuleSetManifest, differing only in the template string literal. Use the parameterized helper function described in the line 233 finding.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:872` — Method RuleResult::passed is byte-identical delegation mirroring ExecutedValidator::passed, differing only in container type. Use the shared trait approach described in the line 504 finding.
- [x] `crates/swissarmyhammer-validators/src/validators/types.rs:877` — Method RuleResult::message is byte-identical delegation mirroring ExecutedValidator::message, differing only in container type. Use the shared trait approach described in the line 504 finding.

Note: The review engine reported 32 findings. The review skill has a written exception: do not record findings that ask for changes to test code that existed before this commit. This rule removed 11 findings: `tests.rs:1810`, `tests.rs:1942`, `tests.rs:2195`, `test_support.rs:125`, `test_support.rs:166`, `test_support.rs:180`, `test_support.rs:245`, `test_support.rs:1429`, `types.rs:947`, `types.rs:1415`, `types.rs:1738`. All of these point at test code that commit d3f1e0a15 did not add. The finding at `test_support.rs:346` stays because it points at the helper `write_tool_rule_ruleset`, which this commit added.