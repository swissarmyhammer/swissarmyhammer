---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kywnch71fjpwv22n70e60q3k
  text: |
    Implementation landed. Changes:

    - `crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs`: `ValidatorSummary.rules: Option<Vec<RuleDetail>>` (skipped when absent), new `rule_details()` shared by `get_validator` and the summary row, `list_validators(source, match, include_rules)`, and `engine_matched_names()` — a path-shaped `match` now resolves through `MatchContext::with_file` + `loader.matching_rulesets`, the engine's own matcher. A glob-fragment `match` keeps the lenient substring behavior.
    - `crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs`: new `rules` boolean ParamMeta, passed to `list_validators`; `scope_for_path` now uses the shared `is_glob_pattern`.
    - `crates/swissarmyhammer-tools/src/mcp/op_tool_helpers.rs`: new `bool_arg` + `is_glob_pattern` (with unit tests) — the module is the declared home for op-tool argument primitives.
    - `crates/swissarmyhammer-validators/src/review/scope.rs`: `engine_matched_validator_names()` behind `cfg(test, feature="test-support")`, a thin wrapper over the private `match_validators_and_files`, re-exported from `review::test_support`. This lets the tool test assert against the ENGINE pairing instead of a re-implementation.
    - `description.md`: documents `rules: true` and the implement-time use.

    Discovery worth keeping: the alignment change is observable. A validator that declares `match.files` AND `match.tools` matches a .rs glob, but the engine never pairs it with a file (a review MatchContext carries no tool name). The old glob-only filter listed it; the engine path does not. The test plants such a fixture (`edit-hook-rules`), and I verified RED by temporarily disabling the engine branch: the tool listed `edit-hook-rules`, the engine did not, and the equality assertion failed. Restored and green.

    Also confirmed: builtin validators load inside `IsolatedTestEnvironment` (embedded, not from the real ~/.validators), so the engine-equality assertion covers the whole loaded stack, not just fixtures.
  timestamp: 2026-07-31T18:01:06.529758+00:00
- actor: claude-code
  id: 01kywt24zgvfxvd08sbws0k4h6
  text: |
    Verification done, ready for review.

    Second gap found and fixed by TDD: an empty `match` string. The old substring filter matched every glob (`g.contains("")`), so `match: ""` listed everything; the engine path treats "" as a path and matched almost nothing. RED proved it (left `[]`, right the full 21-validator list). The dispatch site now treats an empty `match` as absent, the same way an empty `op` falls back to DEFAULT_OP. Test: `list_validators_treats_an_empty_match_as_no_filter`.

    Tests added (all through the registered tool, real loader, real engine):
    - `list_validators_with_rules_pairs_like_the_engine_and_carries_bodies` — name list equals `engine_matched_validator_names(path, loader)`; every row's `rules` is byte-identical to that name's `get validator` rules; the fixture body is the real markdown.
    - `list_validators_omits_rule_bodies_by_default`
    - `list_validators_matches_a_glob_fragment_leniently` (guard for the preserved lenient path)
    - `list_validators_treats_an_empty_match_as_no_filter`
    - `bool_arg_*` / `is_glob_pattern_*` unit tests in `op_tool_helpers`

    Commands run:
    - `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings` — clean.
    - `cargo nextest run -p swissarmyhammer-tools -E '<review/validator tests>'` — 59 passed.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'` — 5012 tests, 5011 passed, 2 skipped, 1 failed: `review_progress_notifications_test::review_working_emits_progress_notifications_per_pair_when_token_supplied` ("notifications/progress regressed ... 55 -> 53").

    That failure is a pre-existing load-dependent flake, not this change: it passes 3/3 in isolation, and an earlier full run of the same command passed it while three llama/GPU tests timed out instead (those three also pass in isolation — model-singleton contention). Nothing in this change touches the progress bridge. Filed as ^aekpq0b: fix the monotonic sequencing in the emitter, never relax the assertion.
  timestamp: 2026-07-31T19:22:49.200404+00:00
position_column: doing
position_ordinal: '8380'
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