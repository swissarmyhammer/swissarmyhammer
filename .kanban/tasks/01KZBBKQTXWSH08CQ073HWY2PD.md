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
position_column: doing
position_ordinal: '8480'
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