---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzcnzj9zwrxdj493e6f2kbzx
  text: |-
    ### Research

    Surfaces and facts found:

    - `swissarmyhammer-validators/src/lib.rs::match_rules` builds `MatchContext::new().with_file(...)` only. No `project_types`, so a `project_types`-keyed set never matches. Its only caller is `crates/swissarmyhammer-tools/tests/validators_engine_smoke_test.rs`.
    - `swissarmyhammer-tools/src/mcp/tools/review/validators.rs::engine_matched_names` has the same gap. `list_validators` and `dump_validators` both route through it.
    - `swissarmyhammer-validators/src/review/scope.rs::detected_project_type_keys` is already `pub` and re-exported as `swissarmyhammer_validators::review::detected_project_type_keys`. It warns and returns an empty vec on detection failure — the fail-closed behavior the card asks to keep.
    - `ReviewTool::resolve_repo_path` already resolves the root from `context.working_dir` (never `current_dir()`) and errors when it is unset. The loader-read ops must NOT error, so a fail-soft `Option<PathBuf>` accessor is the right shape, with `resolve_repo_path` delegating to it.
    - `MatchContext::with_project_types(vec![])` and an unset `project_types` both fail closed (`types.rs` matching tests prove it), so the op can always set the key.

    Discovery, out of scope for this card: `ValidatorLoader::load_all` resolves the PROJECT validator directory with `ManagedDirectory::from_git_root()`, which reads the process CWD, not the session working dir. That is a separate CWD dependency on the same ops.
  timestamp: 2026-08-06T23:19:21.151256+00:00
- actor: claude-code
  id: 01kzcpha7r8m11hgzeq30z4nx6
  text: |-
    Implementation landed. TDD order: both tests written and watched RED first.

    RED evidence:
    - `dump_validators_matches_a_project_types_keyed_validator_only_in_that_workspace` failed with "a rust workspace must dump the `project_types: [rust]` keyed validator: [...]" — the seeded set was absent from the dump.
    - `match_rules_selects_a_project_types_keyed_ruleset_in_a_matching_workspace` failed to compile: `match_rules` took one argument.

    What changed:
    - `swissarmyhammer-validators/src/lib.rs`: `match_rules(file_path, workspace_root: Option<&Path>)` now sets `project_types` on the match context. New public `workspace_project_types(Option<&Path>) -> Vec<String>` is the ONE place a rule-matching surface turns an optional root into match-context project types, through `review::detected_project_type_keys`. No root resolves no types, so a keyed set fails closed.
    - `swissarmyhammer-tools/.../review/validators.rs`: `engine_matched_names` takes `project_types`; `list_validators` and `dump_validators` take `workspace_root` and resolve it once per call through the shared helper.
    - `swissarmyhammer-tools/.../review/mod.rs`: new fail-soft `ReviewTool::workspace_root(context)` reads `context.working_dir` then its git root — never `current_dir()`. `resolve_repo_path` now delegates to it and adds only the "working_dir is unset" error the three `review` ops need. Both loader-read op arms pass the root.

    Fixture note: `review/tests.rs` had three near-copies of the RuleSet writer. They now share one `write_ruleset_with_match(base, name, match_yaml, probes, body)`, and the new `write_project_type_scoped_ruleset` uses it — no new copy.

    Unrelated pre-existing break repaired to reach a green suite: the `mcp` module doc example called `start_mcp_server` with 4 arguments. The function lost its 4th parameter in commit e6a384008 and the example was not updated, so `cargo test -p swissarmyhammer-tools --doc` failed before this card. One argument removed.

    Follow-up card created: ^3e3e0xk — `ValidatorLoader::load_all` still resolves the PROJECT `.validators` directory from the process CWD (`ManagedDirectory::from_git_root()`), not the session working dir.
  timestamp: 2026-08-06T23:29:02.712037+00:00
- actor: claude-code
  id: 01kzcpmzavvmc14rpc1bmzmw7c
  text: |-
    ### implement — changed
    - evidence: 5 files — crates/swissarmyhammer-validators/src/lib.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs, crates/swissarmyhammer-tools/tests/validators_engine_smoke_test.rs (plus the pre-existing doctest repair in crates/swissarmyhammer-tools/src/mcp/mod.rs). `cargo clippy --workspace --all-targets` reports zero warnings. `cargo test -p swissarmyhammer-tools` 1312+7+1+1+8+17+1+1+1+3+2+163+2+8 passed, 0 failed. `cargo test -p swissarmyhammer-validators` 450 passed + 2 doc-tests, 0 failed.
    - next: /review
  timestamp: 2026-08-06T23:31:02.619222+00:00
- actor: claude-code
  id: 01kzcpx6vfzmypfx6sz0fq8g0s
  text: 'Check-in from the watching session (2026-08-06): as the card''s author I confirm both acceptance criteria are met as implemented — project_types-keyed rules appear in dump/list only for a matching workspace, resolved fail-soft from context.working_dir, never current_dir(). The shared `workspace_project_types` single-owner helper is the right shape. Your follow-up ^3e3e0xk is the correct split: same defect class, one layer deeper (the .validators project-layer load). Good catch — that one would have made a project''s own validator overrides silently vanish under a GUI-launched server with CWD=/.'
  timestamp: 2026-08-06T23:35:32.463128+00:00
- actor: claude-code
  id: 01kzcq5jz3v6cxb9qx5cb60yt5
  text: |-
    ### commit — changed
    - evidence: aebdc9d76 feat(validators): resolve project types on hook and list/dump ops (^3hwy2pd)
    - next: review
  timestamp: 2026-08-06T23:40:07.011057+00:00
- actor: claude-code
  id: 01kzcqsp5pe5rsvn0a0743znn4
  text: |
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (aebdc9d76) — 9 attempted, 4 confirmed, 7 refuted, 0 failed, 0 skipped files. All 4 confirmed findings were dropped under the skill's blanket existing-test exception: each asks to rename a literal into a named constant in `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs` (lines 1538, 1921, 1926, 2182), which is `#[cfg(test)]`-only code. `git blame` puts every one of those lines in an earlier commit (0ecaff64a6, e6a3840085, 945a7583ff), and this commit's hunks in that file all end at or below line ~1064, so none of the flagged code was introduced or touched here. The exception names `naming` explicitly. Zero recordable findings; no prior `## Review Findings` section existed.
    - next: none — moved to done
  timestamp: 2026-08-06T23:51:05.654741+00:00
- actor: claude-code
  id: 01kzcqtdfygtkjpkrnrj0v883n
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 6 files; match_rules and the list/dump validators ops take a workspace root, resolved from the session working dir; new card ^3e3e0xk records the same defect one layer down in ValidatorLoader::load_all
    - test: green — cargo nextest run --workspace 13653 passed, cargo test --doc --workspace 0 failed, fmt clean, clippy clean
    - commit: aebdc9d76
    - review: clean — 9 attempted, 4 confirmed, 7 refuted; all 4 dropped under the existing-test exception; task moved to done
  timestamp: 2026-08-06T23:51:29.534218+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffb580
title: Resolve project types on the hook and list/dump validators surfaces
---
Follow-up to ^ygt2rre. Its research note found: `lib.rs::match_rules` (the hook surface) and the `list validators` / `dump validators` ops have no workspace root. They resolve no project types. A `project_types`-keyed rule fails closed there and silently never matches.

Why this matters now: the implement skill calls `dump validators` for rules-up-front. Tool rules key on `project_types` (^q4909tf, ^b01gtzg). Without this fix, the rule dump omits every tool rule — the implementer never sees them, and only the review engine enforces them. That recreates the surprise-findings problem this whole project exists to remove.

Work:
- Thread a workspace root into `match_rules` and the `list/dump validators` ops. Resolve once per call with the same `detected_project_type_keys` helper `scope.rs` uses (^ygt2rre landed it).
- Resolve the root from the session working dir, never `std::env::current_dir()`.
- Fail-closed stays the behavior when no root is available.

Acceptance:
- `dump validators` from a rust workspace includes a `project_types: [rust]` keyed rule; from a non-rust workspace it does not.
- The hook surface matches a project_types-keyed validator in a matching workspace.

#tool-validators