---
assignees:
- claude-code
position_column: todo
position_ordinal: c180
title: 'Review tool: `list validators` returns rule bodies on request (`rules: true`)'
---
# Goal

One call gets the full rules that apply to a target file. This supports the implement skill: read the rules for a file before you edit the file.

# What works today (verified live)

`{"op": "list validators", "match": "crates/swissarmyhammer-skills/src/skill_loader.rs"}` returns the correct 16 validators for a Rust file. The glob test is the engine's own `matches_any_pattern` + `GLOB_MATCH_OPTIONS`, and the loader defaults a missing `match:` to the source-code file group — so the answer agrees with the review engine for full-path queries. But the summaries carry only descriptions. To read the rule bodies, a caller must then call `get validator` once per name (16 calls for one Rust file).

# Changes

1. Add an optional boolean `rules` (default false) to `list validators` in the review tool (crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs). When true, each row also carries the ruleset's rules: each rule `name` + `body` verbatim (same shape as `get validator`).
2. Keep the matching path unchanged — one call with `match: <file>` + `rules: true` returns the full rule text that a review run will enforce on that file.
3. Update the tool description (description.md) with the new field and the implement-time use: get the rules for a file before you edit it.
4. Alignment cleanup while there: in `passes_filters`, prefer the engine's `MatchContext` + `matching_rulesets` path for a path-shaped `match` value, so the tool can never drift from the engine matcher. Keep the documented lenient substring behavior for glob-fragment queries.

# Acceptance

- A production-path test: `list validators` with `match: <a .rs path>` and `rules: true` returns the same ruleset names the engine pairs via `match_validators_and_files` for that path, each with verbatim rule bodies.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'` passes. #review