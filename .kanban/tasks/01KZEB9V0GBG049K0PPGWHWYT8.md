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
position_column: doing
position_ordinal: '8480'
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