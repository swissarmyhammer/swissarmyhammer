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
- actor: claude-code
  id: 01kywy56p5kdpbkrc7xp90eaqa
  text: |
    Fixed the Eq-without-Hash finding at the root (card back in `doing`).

    Change: `Scope` in `crates/swissarmyhammer-validators/src/review/scope.rs` now derives `Hash`:
    `#[derive(Debug, Clone, PartialEq, Eq, Hash)]`. All four variants hold only `String`, so the derive is exact.

    Whole-file sweep: `Scope` is the ONLY type in the file that has `Eq`. A case-sensitive scan for `\bEq\b`, `\bHash\b` and `\bOrd\b` now reports the derive line plus the new test comments and nothing else. The other 10 types in the file (`ScopeSpec`, `WorkList`, `ValidatorWork`, `FileWork`, `GroupedEntities`, `MatchedValidators`, `MatchedValidator`, `FileFacts`, `ResolvedScope`, `FileChangeBuilder`) neither derive nor implement `Eq`, so the rule does not apply to them. No type had to be skipped. No manual `impl Hash for Scope` exists anywhere in the workspace, so the derive cannot conflict.

    TDD: added `review::scope::tests::scope_is_usable_as_a_hash_key`. RED first — it failed to compile with `E0599 ... the following trait bounds were not satisfied: scope::Scope: Hash`. GREEN after the derive. The test proves Hash agrees with Eq: 4 distinct scopes insert, re-inserting `Working` and `Sha("HEAD~1")` is a no-op, and `Glob("a.rs")` stays distinct from `File("a.rs")`.

    Verification:
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 5013 passed, 0 failed, 2 skipped (219s).
    - `cargo fmt --all` — clean, no reformat of the change.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
  timestamp: 2026-07-31T20:34:23.557504+00:00
- actor: claude-code
  id: 01kywz1bp8kkm4h669g88ng36t
  text: |-
    Picked the card back up for the 3 findings from the 2026-07-31 15:35 review. Research first.

    Real locations confirmed (the engine's numbers are stale): the duplicated selector-error prefix is at lines 122 and 128; `read_at_ref` is at line 818.

    Finding 3 — searched the whole workspace for an existing wrapper to reuse before inventing one. There is NONE:
    - No `GitRef` / `RefSpec` / `GitRefSpec` / `CommitIsh` / `Revspec` / `CommitSha` type exists in any crate. Every refspec is a bare `&str`/`String` (`GitOperations::resolve_ref_to_commit`, `DiffScope::Commit{sha:String}`, `CommitInfo{hash:String}`).
    - The closest type is `swissarmyhammer_git::BranchName` (the workspace's only git-domain newtype). It CANNOT be reused as a refspec: its `validate_branch_name` rejects `~ ^ : ? * [ \ ..` — exactly the syntax a refspec needs (`HEAD~3`, `main^`, `HEAD~3..HEAD`). Only `new_unchecked` could hold one, which defeats the type.
    - No repo-relative-path newtype exists either (`FilePath`/`RepoRelativePath`/`RelativePath` are absent; the only `RelativePath` hit is an error-enum variant).

    So new types are required. They follow the prevailing workspace pattern (`BranchName`, `AgentName`, `SkillName`): owned `String`, private field, `new`/`as_str`, `Display`.

    Call-site count for `read_at_ref`: 4 in production (resolve_working, resolve_sha x2, resolve_file) + 3 in tests. Bounded.

    Whole-file duplicate-literal sweep (production half, lines 1..1170): exactly two duplicated literals — the selector-error prefix (2x, as a shared prefix, which is why a naive uniq misses it) and `"HEAD"` (3x). Nothing else repeats.

    Whole-file adjacent-same-typed-parameter sweep: `read_at_ref` is the ONLY production function with two adjacent `&str` parameters. Full accounting in the closing comment.
  timestamp: 2026-07-31T20:49:46.184351+00:00
- actor: claude-code
  id: 01kywzy4vc5ws2c3tfjwajy3mz
  text: |-
    All 3 findings fixed at the root. Every change is in `crates/swissarmyhammer-validators/src/review/scope.rs`; nothing under `builtin/validators/` was touched.

    **Findings 1+2 (one cause, one fix)** — added `const SCOPE_SELECTOR_ERROR_PREFIX: &str = "a review scope must set exactly one of file/glob/working/sha";` beside `SCOPE_VALIDATOR`. `ScopeSpec::resolve` now builds both branches from it: `format!("{SCOPE_SELECTOR_ERROR_PREFIX}; none were set")` and `format!("{SCOPE_SELECTOR_ERROR_PREFIX}; {n} were set")`. Both messages are byte-identical to before.

    **Finding 3** — no reusable wrapper exists in the workspace (see the prior comment), so two new newtypes follow the prevailing `BranchName`/`AgentName`/`SkillName` pattern: owned `String`, private field, `Debug/Clone/PartialEq/Eq/Hash`, `Display`. Both are private to the module — the only consumers are private helpers in this file, so promoting them to `swissarmyhammer-git` public API would add API with no second caller.

    - `GitRefSpec` — `new`, `head()`, `as_str()`. Its doc records why it is NOT `swissarmyhammer_git::BranchName`.
    - `FilePath` — `new`. `Display` only; no `as_str` because nothing needs one (an unused accessor would be dead code).
    - `fn read_at_ref(repo: &GitOperations, refspec: GitRefSpec, path: FilePath)` — exactly the signature the finding names. All 4 production call sites and all 3 test call sites updated.

    **Root-cause sweep, same file**

    1. `"HEAD"` appeared 3x in production. Extracted to `const HEAD_REF`, reached through `GitRefSpec::head()`. Production is now free of the bare literal.
    2. The `{refspec}:{path}` blob-address form was interpolated 3x inside `read_at_ref` (the read plus both error messages). Now composed once into a `spec` local that all three use.
    3. `commit_messages` also took a bare `refspec: &str`; it now takes `&GitRefSpec`, so the refspec concept has ONE representation in the file.
    4. Duplicated-literal sweep, mechanical: extracting every string literal from the production half and counting now reports ZERO literal repeated more than once. Before the change it reported two (the selector prefix as a shared prefix, and `"HEAD"`).
    5. Adjacent-same-typed-parameter sweep, mechanical: parsed all 54 production function signatures and compared each adjacent parameter pair's type. `read_at_ref` was the ONLY function with two adjacent `&str`/`String` parameters, and it is fixed. Three adjacent same-typed pairs remain, none of them `&str`/`String` — reported, not silently refactored, because they fall outside the finding's stated scope:
       - `ValidatorWork::new(rules: Vec<String>, probes: Vec<String>)` — rule names vs probe names. Genuine transposition risk. `pub`, 3 call sites, all inside this crate.
       - `select_probe_results(changed_symbols: &[String], probes: &[String])` — symbol names vs probe names. Genuine transposition risk. Private to this file.
       - `FileChangeBuilder::push(before: Option<String>, after: Option<String>)` — same semantic KIND (file content) on opposite diff sides; a swap reverses the diff. Private to this file.
       Five more functions pair `repo_path: &Path` with `path: &str` (`read_working`, `confine_to_repo`, `resolve_file`, `resolve_sha`, `resolve_glob`); those types already differ, so the compiler rejects a swap and there is no mixup to prevent. Left as is.

    **TDD** — RED first, verified by compile failure for the right reasons: `cargo build --tests -p swissarmyhammer-validators` failed with 12 errors — 2x `E0425: cannot find value SCOPE_SELECTOR_ERROR_PREFIX`, 5x `E0433: cannot find type FilePath`, 5x `E0433: cannot find type GitRefSpec`. GREEN after the implementation.

    Tests added:
    - `both_selector_errors_are_built_from_one_prefix` — asserts each message equals the prefix constant plus its suffix. Mutation-checked: re-introducing a hardcoded literal with a dropped `/sha` in one branch makes it FAIL (left/right diff shown), so the guard really catches drift rather than passing vacuously.
    - `read_at_ref_addresses_the_path_within_the_refspec_never_the_transposition` — reads the blob through the two types, then proves the swapped pair addresses nothing. I verified the real return value rather than assuming: the transposed spec `src/lib.rs:HEAD` yields exactly `Ok(None)`, so the assertion is the strong `matches!(transposed, Ok(None))`, not a weak not-Some.
    - The 3 pre-existing `read_at_ref` tests now construct `GitRefSpec::head()` / `FilePath::new(..)`, so reverting the newtypes breaks compilation.

    **Verification**
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 5015 tests, 5015 passed, 0 failed, 2 skipped (348s). The `review_working_emits_progress_notifications_per_pair_when_token_supplied` flake noted earlier (^aekpq0b) passed this run.
    - `cargo nextest run -p swissarmyhammer-validators` — 330 passed, 0 failed.
    - `cargo fmt --all` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
    - Also confirmed every new intra-doc link resolves: `cargo doc --no-deps --document-private-items` reports only the one PRE-EXISTING warning in this file (`scope_review` → private `auto_purpose`, untouched).
  timestamp: 2026-07-31T21:05:29.452411+00:00
- actor: claude-code
  id: 01kyx0gkz8te798w3jx8ppbgy3
  text: |-
    Behavior-preservation proof, run against `git show HEAD:.../scope.rs` rather than assumed:

    - Both selector messages are byte-identical to the pre-change literals. The old file holds each literal exactly once; the new production half builds each from the constant exactly once, and `PREFIX + "; none were set"` / `PREFIX + "; {n} were set"` reproduce the old strings character for character.
    - `read_at_ref` keeps every arm: the composed address is still `"{refspec}:{path}"`, the `git2::ErrorCode::NotFound` arm still returns `Ok(None)`, the non-blob arm still returns `Ok(None)`, and both failure messages still carry the full address (which is what the non-UTF8 test's path assertion depends on).
    - `resolve_sha` is unchanged in meaning: `from` still feeds the `before` side, `to` the `after` side, the bare-ref case still defaults `to` to HEAD, and `commit_messages` is still called with the `to` ref — confirmed present in BOTH the old and new source.
    - Production now contains the string `"HEAD"` exactly once: the `HEAD_REF` const itself.
    - Nothing outside this file references `read_at_ref`, `GitRefSpec`, `FilePath`, `HEAD_REF`, `SCOPE_SELECTOR_ERROR_PREFIX` or `commit_messages`, so the change has zero blast radius beyond `scope.rs`.

    On the newtype shape: `swissarmyhammer_common::define_id!` was considered and rejected. It emits `pub struct X(pub String)` with a `new()` that mints a fresh ULID and a `Deref<Target = str>` — an ID generator, not a domain-value wrapper. `GitRefSpec::new()` returning a random ULID would be nonsense, the `pub` field drops encapsulation, and the macro would force both types into the crate's PUBLIC API for two private helpers. `BranchName` (private field, purposeful constructors, `Display`) is the right precedent and is what the pair follows. `ARCHITECTURE.md` independently endorses this shape: the focus engine's `FullyQualifiedMoniker`/`SegmentMoniker` are described as "a pair of distinct branded newtypes" whose identity model "eliminates the ... ambiguity that a flat string moniker would otherwise admit" — the same argument as this finding. No `ARCHITECTURE.md` update is needed: no new module, no new component, no new dependency edge.

    A double-check subagent was dispatched but did not return within ~8 minutes, so the checks it was given were performed directly and are the ones listed above.
  timestamp: 2026-07-31T21:15:34.760083+00:00
- actor: claude-code
  id: 01kyx2fad1pjvyvkwfe2p31csq
  text: |-
    Double-check returned REVISE with 5 items. Assessed each on merit; 3 were real and are fixed, 1 was already fixed, 1 was out of scope and is now a card.

    **Accepted — my prefix test was TAUTOLOGICAL (the important catch).** It built its expected value with `format!("{SCOPE_SELECTOR_ERROR_PREFIX}; none were set")` — the same expression the production code uses. Change the constant's VALUE and both sides move together, so it proved nothing about the user-facing text. My earlier mutation test only proved it caught a re-introduced divergent literal, not constant drift. Nothing in the workspace pinned the message text.

    Fixed by strengthening the two pre-existing per-branch tests instead of adding a third (which also answers the "three tests for two branches" item — my new test was a strictly stronger duplicate of both):
    - `scope_spec_errors_on_zero_selectors` now `assert_eq!`s against the HARDCODED full literal, deliberately not composed from the constant. Its doc comment records why.
    - `scope_spec_errors_on_multiple_selectors` now asserts `message.starts_with(SCOPE_SELECTOR_ERROR_PREFIX)` plus the exact suffix, so it carries the shared-prefix claim.
    - Deleted `both_selector_errors_are_built_from_one_prefix` (30 lines).

    Both rot modes are now mutation-proven, each caught by exactly one test:
    - Mutation A, drop `/sha` from the CONSTANT: `..._zero_selectors` FAILS (left/right diff). This is the mode the tautological test missed. `..._multiple_selectors` correctly still passes — prefix-sharing still holds.
    - Mutation B, re-introduce a divergent hardcoded literal in the many branch: `..._multiple_selectors` FAILS with "both selector errors must share one prefix". `..._zero_selectors` correctly still passes.

    **Accepted — `HEAD_REF` was needless indirection.** After the newtype landed the const had exactly ONE use, inside `GitRefSpec::head()`, which is itself the single de-duplicating point. Removed it; `head()` now holds the only `"HEAD"` in production and carries the doc. This reverses part of my earlier sweep note — recording that deliberately.

    **Already fixed before the verdict arrived** — the transposition assertion. The agent read a stale file; it was already `matches!(transposed, Ok(None))`, strengthened after I verified the real return value.

    **Declined, filed instead: ^717c5kc.** The agent argued `FileChangeBuilder::push(path, before, after)` is a WORSE instance of finding 3 than the one fixed — silently transposable, called on the line right after each `read_at_ref`, and inverting Added/Deleted plus before/after content produces a plausible-looking INVERTED diff rather than a nonsense address. I agree with the analysis, but this card's instructions scoped the sweep to `&str`/`String` pairs and directed me to REPORT the remainder for an explicit scope decision rather than refactor unilaterally. So all 3 remaining pairs (`push`, `ValidatorWork::new`, `select_probe_results`) are now card ^717c5kc with the severity argument recorded, which is the non-hiding way to hand over the decision.

    **Full-suite failure investigated, NOT hand-waved: filed as ^8ep9cnf.** One run failed `review::drive::tests::notification_rx_is_the_pools_single_collected_stream`. Evidence: passes 5/5 in isolation; passes 6/6 under 36 CPU burners on 18 cores (so plain CPU contention does not reproduce it); pristine HEAD full suite 5013/5013 passed; full suite re-run WITH this change 5014/5014 passed. Root cause found in `claude_agent::collect_response_content` — it drains notifications by sleeping a FIXED `NOTIFICATION_COLLECTION_DELAY_MS = 500` then `collector.abort()`s, so anything not delivered inside that flat window is silently dropped and `collected_text` comes back truncated. Distinct from ^aekpq0b (that is progress-tick ordering in the bridge; this is the fixed-sleep drain). The card forbids fixing it by lengthening the sleep or relaxing the assertion.

    **Final verification after the revisions**
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — exit 0, **5014 run, 5014 passed**, 2 skipped (336s). 5014 not 5015 because one subsumed test was deleted.
    - `cargo fmt --all` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — **exit 0**, zero warning/error lines.
    - Re-checked the invariants mechanically: `HEAD_REF` gone everywhere including doc links, `"HEAD"` exactly once in production, zero duplicated literals in production, both guard assertions in place.
  timestamp: 2026-07-31T21:49:49.345263+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8680
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

## Review Findings (2026-07-31 15:15)

Scope: a561c5b994767f7c008a476313adb01cba1c9863 (`HEAD~1..HEAD`)

- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:32` — Scope enum derives Eq but not Hash. Types implementing Eq should also implement Hash to maintain consistency and enable use in hash-based collections. Add Hash to the derive macro: `#[derive(Debug, Clone, PartialEq, Eq, Hash)]`.

## Review Findings (2026-07-31 15:35)

Scope: f62cf0884d9aa80c10b36302cb86b2304fa51cef (`HEAD~1..HEAD`)

- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:108` — Repeated error message literal should be extracted to a named constant; the same error-message prefix appears twice (lines 108 and 114) and will require synchronized edits if the scope-selector list ever changes. Extract the common prefix to a named constant: `const SCOPE_SELECTOR_ERROR_PREFIX: &str = "a review scope must set exactly one of file/glob/working/sha";` Then use it: `format!("{}; none were set", SCOPE_SELECTOR_ERROR_PREFIX)` and `format!("{}; {{n}} were set", SCOPE_SELECTOR_ERROR_PREFIX)`.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:114` — Repeated error message literal should be extracted to a named constant; the same error-message prefix appears twice (lines 108 and 114) and will require synchronized edits if the scope-selector list ever changes. Extract the common prefix to a named constant: `const SCOPE_SELECTOR_ERROR_PREFIX: &str = "a review scope must set exactly one of file/glob/working/sha";` Then use it: `format!("{}; none were set", SCOPE_SELECTOR_ERROR_PREFIX)` and `format!("{}; {{n}} were set", SCOPE_SELECTOR_ERROR_PREFIX)`.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:589` — Function `read_at_ref` has adjacent parameters `refspec: &str` (line 590) and `path: &str` (line 591) with different semantic meanings—git reference spec vs file path—and should use distinct newtypes to prevent accidental argument mixup at call sites. Define newtypes—e.g., `struct GitRefSpec(String)` and `struct FilePath(String)`, or use existing type wrappers if available elsewhere in the codebase—and update the signature to `fn read_at_ref(repo: &GitOperations, refspec: GitRefSpec, path: FilePath)` to enforce type safety.

Note on line numbers: the engine's numbers are offset from the current file. The real locations are the duplicated message prefix at lines 122 and 128, and `read_at_ref` at line 818.
