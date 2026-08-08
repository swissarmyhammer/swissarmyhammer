---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzf74aex65esnva2m5mv98rh
  text: |-
    Research: `supersedes` was `Option<String>` in two places — `Rule` and `RuleFrontmatter` (validators/types.rs) — and flowed to four consumers: the suppression plan (`plan_rule_by_health` in review/tool_rules.rs), the doctor row (`ToolRuleStatus.supersedes` + `fallback_note` in doctor.rs), the review report note (`render_tool_fallbacks` in review/synthesize.rs), and the MCP `RuleDetail` (tools/review/validators.rs, both the JSON field and the `dump validators` markdown).

    Design: a new public newtype `Supersedes(Vec<String>)` in validators/types.rs. A private `#[serde(untagged)]` helper enum `SupersedesFrontmatter { One(String), Many(Vec<String>) }` plus `#[serde(from = ..., into = "Vec<String>")]` makes both frontmatter shapes parse into one list. A prompt rule holds an empty list, so `Option` is gone from the domain.

    Reader-facing phrasing lives in ONE place: `Supersedes::prompt_rule_phrase()` gives `prompt rule 'a'` for one name and `prompt rules 'a', 'b'` for more; `runs_verb()` gives the agreeing verb. Doctor and the report each add their own tense, so the single-name text is byte-identical to before ("prompt rule 'missing-docs' runs instead" / "... ran instead") and the existing assertions still hold.

    Note on the MCP contract: `RuleDetail.supersedes` now serializes as a JSON array (`["missing-docs"]`), not a bare string, and is skipped when empty. `dump validators` markdown still reads `Supersedes: missing-docs` (comma-joined `Display`).
  timestamp: 2026-08-07T22:57:31.613793+00:00
- actor: claude-code
  id: 01kzf74mpgn8c42pad1a3swxbh
  text: |-
    Red-green record:

    1. RED — added `test_parse_tool_rule_supersedes_list` (parser.rs) against the old `Option<String>` field, asserting only that the list form parses. It failed for the right reason: `failed to deserialize frontmatter: invalid type: sequence, expected a string`.
    2. GREEN — added `Supersedes`, then extended the same test to assert the two names.
    3. The suppression test (`plan_suppresses_every_named_prompt_rule_per_file`) could not fail before the type existed, so its teeth were proven by mutation: with the planner loop cut to `.take(1)` it failed (`left: {"missing-docs"}`, `right: {"function-length", "missing-docs"}`); the loop was restored and it passes.

    Tests added: parser list form (parser.rs), suppression of every named rule per file (tool_rules.rs), doctor row naming both rules (`test_to_checks_missing_tool_names_every_superseded_rule`, doctor.rs), report note naming both rules (`a_tool_fallback_names_every_superseded_prompt_rule`, synthesize.rs). The single-name parser test and the prompt-rule test stay, so existing rules are pinned to parse unchanged.

    Verified: `cargo nextest run -p swissarmyhammer-validators` — 523 passed. `cargo nextest run -p swissarmyhammer-tools mcp::tools::review` — 69 passed. `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.
  timestamp: 2026-08-07T22:57:42.096252+00:00
- actor: claude-code
  id: 01kzf74we4dn7103xr5atns62m
  text: |-
    ### implement — changed
    - evidence: 12 files — crates/swissarmyhammer-validators/src/validators/types.rs, validators/parser.rs, validators/loader.rs, validators/mod.rs, doctor.rs, builtin/mod.rs, review/tool_rules.rs, review/synthesize.rs, review/drive.rs; crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs, review/tests.rs; builtin/validators/README.md
    - next: /review
  timestamp: 2026-08-07T22:57:50.020757+00:00
- actor: claude-code
  id: 01kzfhfmdqgrexxq8f0b32tf1f
  text: |-
    ### test — green
    - evidence: `cargo check --workspace --all-targets` clean; `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo nextest run --workspace` — 13782 passed, 0 failed, 0 skipped.
    - blast-radius check: grepped whole repo for `supersedes` (case-sensitive) outside `swissarmyhammer-validators` and `swissarmyhammer-tools`. Hits in `crates/swissarmyhammer-kanban/benches/move_task_bench.rs` and `crates/mirdan/src/install/profile.rs` / `edit_redirect_tests.rs` are plain-English comments ("X supersedes Y"), not uses of `Rule.supersedes` or the `Supersedes` type. No other crate references the changed field or type.
    - next: none, workspace is green.
  timestamp: 2026-08-08T01:58:28.023064+00:00
- actor: claude-code
  id: 01kzfk29rgncaqbwz2hz50tt3f
  text: |
    ### review — findings
    - evidence: 2 open findings — crates/swissarmyhammer-validators/src/doctor.rs:1160, crates/swissarmyhammer-validators/src/validators/parser.rs:189
    - scope: review sha 035cf5d0a (HEAD~1..HEAD)
    - engine counts: 5 findings, 5 confirmed, 9 refuted, 54 attempted, 0 failed
    - dropped: 3 findings by the skill rule that drops refactors of test code that already existed — drive.rs:1152 (named constant for a chunk count), tool_rules.rs:1096 (named constant for an exit code), loader.rs:632 (extract the EnvVarGuard test helper). Each line is pre-existing and sits in a #[cfg(test)] module.
    - next: implement the 2 open findings, then review again
  timestamp: 2026-08-08T02:26:08.272990+00:00
- actor: claude-code
  id: 01kzfk39rt334nxfhfz86sg2w8
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 12 files
    - test: green — cargo nextest run --workspace, 13782 passed, 0 failed, 0 skipped; fmt, clippy -D warnings clean
    - commit: 035cf5d0a feat(validators): supersedes accepts one name or a list (^gwhwyt8)
    - review: findings — crates/swissarmyhammer-validators/src/doctor.rs:1160, crates/swissarmyhammer-validators/src/validators/parser.rs:189
    - engine counts: 5 confirmed, 9 refuted, 54 attempted, 0 failed; 3 dropped by the written test-code exception
  timestamp: 2026-08-08T02:26:41.050735+00:00
- actor: claude-code
  id: 01kzfk9jbn0fsc1p9mhnp2mn4t
  text: |-
    Research for the 2 open review findings.

    Finding 1 (doctor.rs fallback note, singular form). The note is built by `fallback_note` in doctor.rs from `Supersedes::prompt_rule_phrase()` + `Supersedes::runs_verb()`; the grammatical agreement lives in `Supersedes::is_one()` / `rule_noun()` / `runs_verb()` in validators/types.rs. The singular fixture is `MISSING_TOOL_RULE` (`supersedes: missing-docs`), so the exact singular note is `; prompt rule 'missing-docs' runs instead`. I grepped doctor.rs for every assertion on the note: line 1161 (`contains("missing-docs")`) is the ONLY weak one. The empty-supersedes assertion (`contains("prompt fallback")`) already matches the whole note text, and line 911 is an exact `assert_eq!` on the parsed names. So the cause appears once in this file.

    Finding 2 (parser.rs owned `PathBuf`). Three functions take `path: PathBuf`, not one: `parse_validator`, `parse_validator_with_expansion` (the line the finding names), and the shared `parse_validator_internal` they both call. Every other path-accepting function in the file already takes `&Path` — `parse_rule`, `parse_ruleset_manifest`, `check_manifest_frontmatter`, `extract_frontmatter`, `require_ruleset_layout`, `is_rule_file`, `parse_rule_in_set`. So all three move to `&Path`; `Validator.path` stays owned and is filled with `path.to_path_buf()` inside `parse_validator_internal`.

    Blast radius: the only production caller of any of the three is `ValidatorLoader` in validators/loader.rs — `parse_and_insert_validator` (which today calls `path.to_path_buf()` at the call site, so the conversion just goes away) and `add_builtin` (which builds a fresh `PathBuf` and now binds it before passing a reference). `parse_validator` itself has no caller outside parser.rs's own tests. No other crate names these functions.
  timestamp: 2026-08-08T02:30:06.453944+00:00
- actor: claude-code
  id: 01kzfkjz7xake0n8y473v3592p
  text: |-
    Both review findings worked. Detail on each, and the red-green proof.

    Finding 1 — the singular fallback note is undertested (doctor.rs). I proved the gap before I closed it. I cut `Supersedes::is_one()` down to `false`, which is exactly the defect the finding describes ("always returning 'rules' and 'run' regardless of count"), then wrote the strengthened assertion. RED, and for the predicted reason: `a missing tool row must name the one prompt rule ... got 'tool missing: ...; prompt rules 'missing-docs' run instead'`. The plural test `test_to_checks_missing_tool_names_every_superseded_rule` PASSED against that same broken code — so the finding was right that the plural test alone cannot catch a broken agreement. Restoring `is_one()` to `self.0.len() == 1` turned it GREEN. The assertion now pins the whole singular note, `prompt rule 'missing-docs' runs instead`, not the bare `missing-docs` substring.

    I swept the rest of doctor.rs for the same cause. Line 1161 was the only weak assertion on the note: the empty-supersedes case already asserts its whole note text (`prompt fallback`), and the parse assertion is an exact `assert_eq!` on the names.

    Finding 2 — owned `PathBuf` where `&Path` suffices (parser.rs). The finding named line 189, but the cause sat in THREE functions, so all three moved to `&Path`: `parse_validator`, `parse_validator_with_expansion`, and the shared `parse_validator_internal`. `Validator.path` is still owned, filled with `path.to_path_buf()` at the one place the struct is built. parser.rs now has no function that takes an owned path; every path-accepting function in the file reads `&Path`, matching `parse_rule` and the rest.

    Callers, all updated: loader.rs `parse_and_insert_validator` LOST a `path.to_path_buf()` at the call site (the copy the old signature forced), and `add_builtin` binds its constructed path and passes a reference — its redundant function-local `use std::path::PathBuf` went away, since the module already imports it. The 15 test call sites in parser.rs moved from `PathBuf::from("...")` to `Path::new("...")`.

    Blast radius: `parse_validator` is re-exported from validators/mod.rs, so this is a public API signature change — the one the finding asked for, with every caller in the diff. `cargo check --workspace --all-targets` is the authoritative check here (the compiler sees every caller) and it is clean; a repo-wide grep finds no other reference to the three functions.

    Verified: `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo nextest run -p swissarmyhammer-validators` 523 passed, 0 failed, 0 skipped; `cargo nextest run -p swissarmyhammer-tools mcp::tools::review` 69 passed.
  timestamp: 2026-08-08T02:35:14.557997+00:00
- actor: claude-code
  id: 01kzfkk4m9h3zm00cnb93hfa0z
  text: |-
    ### implement — changed
    - evidence: 3 files — crates/swissarmyhammer-validators/src/doctor.rs, crates/swissarmyhammer-validators/src/validators/parser.rs, crates/swissarmyhammer-validators/src/validators/loader.rs. Both review findings closed and checked off. fmt clean; clippy --workspace --all-targets -D warnings clean; swissarmyhammer-validators 523 passed; swissarmyhammer-tools mcp::tools::review 69 passed.
    - next: /review
  timestamp: 2026-08-08T02:35:20.073359+00:00
- actor: claude-code
  id: 01kzfm3hcxr0drxak82c7b35j0
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13782 tests run: 13782 passed, 0 failed, 0 skipped (324.101s). `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean (0 warnings).
    - context: verified the whole workspace after the `parse_validator`/`parse_validator_with_expansion`/`parse_validator_internal` signature change (PathBuf → &Path) in `crates/swissarmyhammer-validators/src/validators/parser.rs`, its re-export in `validators/mod.rs`, callers in `loader.rs`, and the stricter doctor.rs assertion.
    - next: none — build is clean, ready for the next step.
  timestamp: 2026-08-08T02:44:17.437009+00:00
- actor: claude-code
  id: 01kzfmhghephvthq1r9sny0ask
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (d01050a61) — 0 findings, 0 confirmed, 12 refuted, 18 attempted, 0 failed, 0 skipped. Both prior findings (doctor.rs:1160 singular fallback note, parser.rs:189 `&Path` signature) are checked.
    - next: task moved to `done`.
  timestamp: 2026-08-08T02:51:55.310154+00:00
- actor: claude-code
  id: 01kzfmn1aeyxh5wvjgknd8x5a9
  text: |
    ### finish iteration 2 — clean
    - implement: changed — 3 files, both findings closed and checked
    - test: green — cargo nextest run --workspace, 13782 passed, 0 failed, 0 skipped; fmt, clippy -D warnings clean
    - commit: d01050a61 fix(validators): pass Path not owned PathBuf, pin doctor fallback note (^gwhwyt8)
    - review: clean — 18 validator pairs attempted, 0 findings, 12 refuted
    - task moved to done by the review gate
  timestamp: 2026-08-08T02:53:50.798409+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffc580
title: 'validators: `supersedes` accepts one name or a list'
---
Change `Rule.supersedes` from `Option<String>` to a one-or-many value.

Why: one workspace tool run can replace more than one prompt rule. One `cargo clippy` run finds cognitive complexity and long functions. That rule must supersede `cognitive-complexity` AND `function-length`.

Requirements:
- Frontmatter accepts `supersedes: name` and `supersedes: [a, b]`. Both parse.
- The suppression plan inserts one entry per named rule per matched file.
- Doctor rows and the fallback note show every named rule.
- The README tool-rule section states the list form.
- Existing single-name rules parse unchanged. Add a parser test for each form. #tool-validators

## Review Findings (2026-08-07 21:04)

- [x] `crates/swissarmyhammer-validators/src/doctor.rs:1160` — The fallback note format for singular supersedes is undertested. The new test at line 1199 explicitly verifies the plural format ('prompt rules ... run instead'), but the existing test at line 1160-1164 only checks that 'missing-docs' appears as a substring when a single rule is superseded. If `prompt_rule_phrase()` and `runs_verb()` methods don't handle grammatical agreement correctly (e.g., always returning 'rules' and 'run' regardless of count), the singular case would produce 'prompt rules 'missing-docs' run instead' (grammatically incorrect), but this test would silently pass. Add an explicit assertion in test_to_checks_missing_tool_is_warning_with_install_fix (around line 1160) to verify the exact singular form of the fallback note, e.g., assert that the message contains a phrase like 'prompt rule 'missing-docs' runs instead' (or whatever singular form is intended), mirroring the rigor of the plural test at line 1199.
- [x] `crates/swissarmyhammer-validators/src/validators/parser.rs:189` — Function accepts `PathBuf` instead of `&Path`, requiring callers to own the path when a reference would suffice. Violates the principle 'Accept generics, not concrete types' and is inconsistent with other path-accepting functions in the same module. Change to `path: &Path`. If `Validator` requires owned path, convert internally with `path.to_path_buf()`.
