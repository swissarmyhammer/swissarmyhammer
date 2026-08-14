---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzxhnkxhsvhg8m4mbqjgcp1s
  text: |-
    Implemented, following commit 59f31b4f6 (sibling `^w6ypb8b`) exactly.

    ## Snapshots — byte-identity verified

    Each file recovered with `git show 59bd9ae5c~1:<path>`, written under
    `crates/mirdan/retired-validators/<set>/fixtures/`, then compared against the
    git blob with `cmp` and `sha256`. All four are IDENTICAL:

    - `e1bac79977102540320ba153af8e77c7eded70435e00be3a29864574a4bb490d`
      code-hygiene/fixtures/no-commented-code-parsed.fail.rs.tmpl (434 bytes)
    - `cc033268a5aa0df9528e755eb19fff14ca4d1cb6e5a6a85b27a26bea1cc457a5`
      code-hygiene/fixtures/no-commented-code-parsed.pass.rs.tmpl (1375 bytes)
    - `2d041798abda67315b9603eda2d32204363448ac22a48507f592f1448422acad`
      duplication/fixtures/duplication-parsed.fail.rs.tmpl (1406 bytes)
    - `65709779c95d7daf96bbecd56d37e5eef761cc9327ebee380ec3fa5a21035c7d`
      duplication/fixtures/duplication-parsed.pass.rs.tmpl (3742 bytes)

    The check was run again after all mutation testing, and gives the same four
    hashes.

    ## The empty `duplication/fixtures/` directory does NOT matter

    `59bd9ae5c` removed `builtin/validators/duplication/fixtures/` entirely because
    it became empty, while `code-hygiene/fixtures/` still ships many files. The
    file-grain prune removes files only, so a deployed store keeps an empty
    `duplication/fixtures/` after the prune. That costs nothing, and the reason is
    structural, not incidental:

    - `swissarmyhammer-validators/src/validators/parser.rs::require_ruleset_layout`
      is the whole set-layout contract: `VALIDATOR.md` plus `rules/`. `fixtures/`
      is not checked at all, so an empty one cannot make a set malformed.
    - The loader reads the store root one level deep and needs `VALIDATOR.md`, so
      `<set>/fixtures/` is never a set candidate and never a phantom rule.
    - Both readers of a fixtures directory — `doctor.rs::find_fixture` and
      `review/tool_health.rs::fixture_digest` — answer identically for an empty and
      an absent directory.
    - `loader.rs::fixture_dirs` computes `base_path.join("fixtures")` whether or
      not the directory exists, and the review-scope exclusion is a path prefix
      test, never a listing.

    Confirmed by the real binary: after `sah init user` the directory is present
    and empty, and doctor is unaffected.

    ## Report wording corrected

    `install_profile_validators` labelled this prune "retired validator rule(s)".
    With four fixtures now flowing through it, that message named a fixture as a
    rule. Changed to "retired validator file(s)" — the vocabulary
    `RETIRED_VALIDATOR_FILES` and `prune_unmodified_retired_files` already use. No
    test asserted the old string.

    ## Proof

    Real binary, `sah init user`, throwaway HOME (the user's real `~/.validators/`
    was never touched):

    - Store holding all four stale fixtures: reports
      "Removed retired validator file(s): ..." naming all four, and all four are
      gone; the still-shipping sibling `code-hygiene/fixtures/missing-docs-rust.fail.rs.tmpl`
      is present.
    - Store with one fixture edited: the edited copy survives byte for byte, the
      other three are pruned.

    Both halves are also covered by tests, and each was proved load-bearing by
    mutation.
  timestamp: 2026-08-13T12:31:06.161406+00:00
- actor: claude-code
  id: 01kzxhp28a47zqkza6astj34bq
  text: |-
    ## Mutation proof — every new test is load-bearing

    `^w6ypb8b` inherited a test that passed even when the prune did nothing. Each
    new test here was mutated to check it cannot do the same. Every mutation was
    reverted after measuring.

    Mutation A — `prune_unmodified_retired_files` skips every `fixtures/` entry
    (the prune exists but never fires for a fixture):

    - `init_profile_refresh_prunes_all_four_unmodified_retired_fixtures` FAILS at
      "an unmodified retired fixture must be pruned by refresh".
    - `init_profile_refresh_keeps_a_user_modified_retired_fixture` FAILS at
      "an unmodified retired fixture must still be pruned". This is exactly the
      assertion the inherited test lacked, so the "keeps that one" half cannot
      pass on a dead prune.

    Mutation B — the byte comparison is replaced by `true`, so the prune removes a
    file the user edited:

    - `init_profile_refresh_keeps_a_user_modified_retired_fixture` FAILS at
      "a user-modified retired fixture must never be pruned".

    Mutation C — one fixture entry removed from `RETIRED_VALIDATOR_FILES`:

    - `test_retired_files_are_the_two_parsed_rules_and_their_four_fixtures` FAILS.
    - `test_every_retired_tool_rule_has_its_fixtures_retired_with_it` FAILS
      (1 fixture, expected 2).
    - `init_profile_refresh_prunes_all_four_unmodified_retired_fixtures` FAILS.

    Order of work was RED first: the four tests were written and watched fail
    before any snapshot or table entry existed.

    ## Green

    - `cargo nextest run -p mirdan` — 516 passed, 0 failed, 0 skipped, including
      `test_no_retired_file_is_still_shipped_by_a_builtin_set`.
    - `cargo nextest run --workspace` — 14176 passed, 0 failed, 0 skipped.
    - `cargo fmt --check` clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` clean.

    Blast radius: `RETIRED_VALIDATOR_FILES` and `prune_unmodified_retired_files`
    are referenced only inside `crates/mirdan` (`retired_validators.rs`,
    `install/profile.rs`, `install/tests.rs`). No other crate reads either.
  timestamp: 2026-08-13T12:31:20.842475+00:00
- actor: claude-code
  id: 01kzxhpb6r979bgwx4hne3kjaa
  text: |-
    ### implement — changed
    - evidence: 7 files — crates/mirdan/retired-validators/code-hygiene/fixtures/no-commented-code-parsed.fail.rs.tmpl (new), crates/mirdan/retired-validators/code-hygiene/fixtures/no-commented-code-parsed.pass.rs.tmpl (new), crates/mirdan/retired-validators/duplication/fixtures/duplication-parsed.fail.rs.tmpl (new), crates/mirdan/retired-validators/duplication/fixtures/duplication-parsed.pass.rs.tmpl (new), crates/mirdan/src/retired_validators.rs, crates/mirdan/src/install/profile.rs, crates/mirdan/src/install/tests.rs. Workspace 14176 tests pass; fmt and clippy clean.
    - next: /review
  timestamp: 2026-08-13T12:31:30.008075+00:00
- actor: claude-code
  id: 01kzxjf3n68vxct0q9jtefhq4h
  text: |-
    ### review — findings
    - evidence: 3 findings — crates/mirdan/src/install/profile.rs:647, crates/mirdan/src/install/profile.rs:1027, crates/mirdan/src/install/profile.rs:1276. Engine counts on `review sha 99f3229df~1..99f3229df`: 4 findings, 4 confirmed, 3 refuted, 16 attempted, 0 failed, 0 skipped.
    - verification: each named line was read before it was recorded. All four premises are true — every named function does take the `&str` parameters the finding names. 0 findings were dropped for a false premise. 0 findings attacked a frozen fixture; the engine raised nothing against the four snapshots under `crates/mirdan/retired-validators/*/fixtures/`. 1 finding was dropped: `crates/mirdan/src/install/tests.rs:745` (`make_local_skill`), test code that already existed, dropped under the review skill's existing-tests exception.
    - note: all three recorded findings sit on code this commit never touched. `git blame` at 99f3229df puts lines 647, 1027 and 1276 on commit 236d021f8. This commit changed only two hunks in profile.rs — a comment, and the report label "retired validator rule(s)" to "retired validator file(s)".
    - next: /implement — add the newtypes named in each finding, and remove the same cause everywhere in profile.rs, not only at the three named lines.
  timestamp: 2026-08-13T12:45:01.478578+00:00
- actor: claude-code
  id: 01kzxjgdbhbzk6jwgn96zmwd2s
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 7 files. The four fixtures added to RETIRED_VALIDATOR_FILES as byte-frozen snapshots, each verified byte-identical to the 59bd9ae5c~1 blob by cmp AND sha256 (e1bac799… 434 bytes, cc033268… 1375, 2d041798… 1406, 65709779… 3742), re-checked after mutation testing. The prune mechanism itself was untouched.
    - The empty `duplication/fixtures/` against the still-shipping `code-hygiene/fixtures/` costs nothing, and the reason is structural rather than incidental: `require_ruleset_layout` checks VALIDATOR.md and rules/ only, both readers of a fixtures directory answer identically for empty and absent, and `fixture_dirs` computes the path either way. Recorded in the table's doc comment.
    - One label changed beyond the table: `install_profile_validators` said "retired validator rule(s)", which named a fixture as a rule once fixtures flowed through it. Now "retired validator file(s)". No test asserted the old string.
    - Proved with the real binary in a throwaway HOME: a store holding all four loses all four while the still-shipping sibling fixture survives; a store with one edited keeps that one byte for byte and loses the other three. Each test proved load-bearing by mutation, including the exact gap ^w6ypb8b hit.
    - test: green — workspace 14176 tests pass, 0 skipped. fmt and clippy clean.
    - commit: 99f3229df
    - review: findings — crates/mirdan/src/install/profile.rs:647, :1027, :1276. Zero findings fired against any of the four frozen snapshots, so nothing had to be set aside on that ground, and zero were dropped for a false premise. One was dropped under the written existing-tests exception (install/tests.rs:745, `make_local_skill`, blamed to 236d021f8).

    All three recorded findings sit on production code this commit never touched — `git blame` at 99f3229df attributes lines 647, 1027 and 1276 to commit 236d021f8. The commit's only hunks in that file are a comment block and the label change. The existing-tests exception does not reach production code, so they stand as requirements and will be worked.
  timestamp: 2026-08-13T12:45:44.177196+00:00
- actor: claude-code
  id: 01kzxkt73rn39jtfb6t4z13naw
  text: |-
    All three review findings worked. Each one is now `- [x]`.

    ## The premises hold — nothing was dropped

    Each named function was read before any edit. All four premises stated by the
    reviewer are true: `stage_and_deploy_rendered` took `name`, `content` and
    `file_name` as `&str`; `apply_mcp_operation_at` took `server_name`, `verb` and
    `preposition` as `&str`; `run_deinit_step` took `component` and `kind` as
    `&str`. No finding had a false premise, so none was recorded as a conflict.

    All three sit on production code this commit never touched — `git blame` at
    99f3229df puts every named line on 236d021f8. The review skill's existing-code
    exception covers TESTS only, so they stand as requirements.

    The workspace `type-safety` rule states the same requirement independently:
    "Newtypes for semantic distinctions. Two parameters of the same primitive type
    with different meanings must use newtypes."

    ## The idiom mirdan already has

    mirdan does not need a new newtype spelling. `crates/mirdan/src/mcp_config.rs`
    carries a `string_newtype!` macro whose own doc comment names this exact
    problem: keeping `ServersKey` apart from `ToolName` "is what stops the two from
    being passed in the wrong order". Every new type here comes out of that macro,
    so all seven get the same six impls from one source.

    Two supporting changes to make the macro reusable:

    - `pub(crate) use string_newtype;` in `mcp_config.rs` makes it reachable by
      path from `install/profile.rs`.
    - Its `Display` impl now expands to `::std::fmt::...` instead of a bare `fmt`.
      It previously leaned on the caller module's `use std::fmt;`, which is why the
      first build failed with 21 `cannot find module fmt` errors at the new call
      sites. The `use std::fmt;` in `mcp_config.rs` was its only user and is gone.

    ## One deviation from the finding's literal wording

    Finding 2 names `struct ServerName(String)`. `ToolName` already IS that type —
    same macro, documented as "the name one MCP server is registered under" — and
    `apply_mcp_operation_at` was already converting its `&str` into
    `ToolName::new(server_name)` inside the closure. A second type for one concept
    is the drift the macro exists to prevent, so `server_name` became `&ToolName`
    and the inner conversion disappeared. The finding's requirement (a distinct
    type that blocks the swap) is met. The other six newtypes use the exact names
    the findings gave: `ItemName`, `ManifestContent`, `ManifestFilename`,
    `ActionVerb`, `Preposition`, `ComponentName`, `ItemKind`.

    ## How far the cause was removed — measured

    The cause is a function that takes two or more parameters of the same type, so
    a caller can hand them over in the wrong order with no compiler complaint. A
    script scanned every `fn` signature in profile.rs for it.

    Before: 4 functions. Three are the findings; the fourth,
    `deinit_profile_items`, has the same `component: &str, kind: &str` pair as
    `run_deinit_step` and was not named. It is fixed too.

    After: 0 functions. The scan reports none.

    Beyond the four, three more functions changed so one concept keeps one
    spelling:

    - `register_mcp_server_at` and `unregister_mcp_server_at` take `&ToolName`,
      because `apply_mcp_operation_at` now requires one.
    - `run_install_step` takes `ComponentName`. It carries a single `&str` and
      therefore has no swap hazard, but it is the install twin of
      `run_deinit_step`; leaving `component: &str` beside `component: ComponentName`
      in one file would be a new inconsistency.

    Four functions keep a single `&str` parameter and were left alone:
    `render_profile_agent` (`name`), `stage_validator_set` (`set`),
    `remove_builtin_file_and_cleanup` (`embedded_name`), `run_install_step` before
    the change. A lone parameter cannot be exchanged with anything.

    Struct fields were also left alone, and the reason is structural rather than a
    judgment call: `ProfileItemKind` holds three `&'static str` fields and
    `SettingsFragment` holds two, but a Rust struct literal must spell each field
    name, so no value can silently reach the wrong field.

    ## Behavior is unchanged, on purpose

    Every reporter string and every error message is byte-identical. The Debug
    format of `format!("unsafe name: {name:?}")` would have become
    `ItemName("x")`, so it is now `{:?}` of `name.as_str()`.

    The three staging arguments are passed by value with `From`, not `new`, so the
    rendered manifest body is moved rather than cloned per item.
  timestamp: 2026-08-13T13:08:34.040615+00:00
- actor: claude-code
  id: 01kzxktrh1rcsfa9gg18wc6h0s
  text: |-
    ## The two new tests, and the proof they are load-bearing

    A newtype refactor must not change behavior, so the guard is a test that pins
    the behavior the swap would break. Two slots had no such test, and both were
    written and run BEFORE any production edit:

    - `deinit_profile_reports_each_family_under_its_own_component_and_kind` — the
      `component` and `kind` pair, asserted as `profile-skills` /
      `Removed 1 skill(s)` and `profile-agents` / `Removed 1 agent(s)`.
    - `profile_mcp_root_explicit_reports_its_own_verb` — the `verb`, asserted as
      `Registered applied to 1 agent(s)` on init and `Removed applied to 1 agent(s)`
      on deinit. It runs with an explicit root, because `root: None` dispatches
      through the strategy applier and never reaches `apply_mcp_operation_at`.

    Both passed against the unchanged code, then each was proved to fail on the
    swap it guards. Mutations applied to the old `&str` call sites:

    - `run_deinit_step` given `"skill", "profile-skills"` — FAILS with
      `profile-skills result must be present: [... InitResult { name: "skill",
      message: "Removed 1 profile-skills(s)" } ...]`.
    - `register_mcp_server_at` given `"for", "Registered"` — FAILS on the verb
      assertion.

    Both mutations were reverted (`git checkout`) before the refactor started.

    The third finding's three arguments needed no new test:
    `init_profile_installs_and_deinit_removes_cwd_rooted` already asserts
    `.skills/commit/SKILL.md` exists and that the deployed body has no
    `{% include` left. Exchanging `name` with `file_name` writes
    `.skills/SKILL.md/commit`, and exchanging `content` with either writes the
    wrong bytes; the existing assertions catch each.

    ## Green

    - `cargo nextest run -p mirdan` — 518 passed, 0 failed, 0 skipped (516 before,
      plus the two new tests).
    - `cargo nextest run --workspace` — 14178 passed, 0 failed, 0 skipped.
    - `cargo fmt --check` clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` clean.

    Blast radius: eight workspace members depend on mirdan (mirdan-cli, kanban-cli,
    shelltool-cli, code-context-cli, kanban-app, mirdan-app, swissarmyhammer-cli,
    swissarmyhammer-tools). Every function whose signature changed is private to
    `install::profile`, and `grep` finds no caller outside that file, so no
    dependent could see the change. The workspace run and workspace clippy confirm
    it.

    Nothing outside `crates/mirdan/src` was touched: `git status` lists exactly
    `install/profile.rs`, `install/profile_tests.rs` and `mcp_config.rs`. The four
    byte-frozen snapshots under `crates/mirdan/retired-validators/` are unmodified,
    and the real `~/.validators/` was never written.
  timestamp: 2026-08-13T13:08:51.873374+00:00
- actor: claude-code
  id: 01kzxkv0ja5kc93h5aht966p1d
  text: |-
    ### implement — changed
    - evidence: 3 files — crates/mirdan/src/install/profile.rs, crates/mirdan/src/install/profile_tests.rs, crates/mirdan/src/mcp_config.rs. All 3 review findings are `- [x]`. Functions in profile.rs with two or more same-typed parameters: 4 before (the 3 named plus `deinit_profile_items`), 0 after. `cargo nextest run -p mirdan` 518 passed / 0 failed / 0 skipped; `cargo nextest run --workspace` 14178 passed / 0 failed / 0 skipped; fmt and clippy clean.
    - next: /review
  timestamp: 2026-08-13T13:09:00.106757+00:00
- actor: claude-code
  id: 01kzxmgb1jdzgfc0gg1e3zqnqq
  text: |-
    ### review — findings
    - evidence: 2 findings (2 confirmed, 4 refuted, 8 attempted) — crates/mirdan/src/install/profile.rs:603, crates/mirdan/src/install/profile.rs:1173. Scope `f1601a77d~1..f1601a77d`.
    - prior round: all 3 findings checked; the `string_newtype!` sweep removed the transposition cause from the whole file.
    - premise check: both new findings read at the named lines and confirmed. 0 findings dropped for a false premise.
    - next: fix both findings; task stays in review.
  timestamp: 2026-08-13T13:20:38.962163+00:00
- actor: claude-code
  id: 01kzxmhh63jtfe69v6pd0x6b2r
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 3 files. All three prior findings worked and checked. mirdan's existing `string_newtype!` macro was used rather than a new mechanism; its own doc comment names this exact problem. Two supporting changes made it reusable: `pub(crate) use`, and a `Display` impl expanding to fully-qualified `::std::fmt` paths instead of leaning on the caller module's `use std::fmt` (which broke the first build with 21 errors).
    - One deliberate deviation from the finding text: finding 2 asked for a new `ServerName`, but `ToolName` already is that type from the same macro and the function was already calling `ToolName::new` inside its closure. A second type for one concept is the drift the macro exists to prevent, so `server_name` takes `&ToolName`.
    - The whole file was swept rather than the three named lines: a script over every signature counted 4 such functions before and 0 after. The fourth, `deinit_profile_items`, carries the same component/kind pair and no finding named it. Functions with a lone `&str` were left alone, since one parameter cannot be transposed; struct fields were left alone because a Rust struct literal must spell each field name.
    - Two slots had no test pinning them. Both were written and passing BEFORE any production edit, then proved load-bearing by mutating the old `&str` call sites — transposing "profile-skills" with "skill" fails, transposing "Registered" with "for" fails. Behaviour is byte-identical throughout.
    - test: green — 518 mirdan tests, 14178 workspace tests, 0 failed, 0 skipped. fmt and clippy clean.
    - commit: f1601a77d
    - review: findings — crates/mirdan/src/install/profile.rs:603, crates/mirdan/src/install/profile.rs:1173. 0 dropped for a false premise; the reviewer read both named lines and both premises hold.

    The second finding is a genuine miss from the sweep, not a false positive: `"profile-validators"` is wrapped as `ComponentName::new` at 1173 but stands as a bare `&str` at 1263, while the adjacent agents branch wraps its own. One concept, two spellings — exactly the drift the newtype was introduced to stop.
  timestamp: 2026-08-13T13:21:18.019777+00:00
- actor: claude-code
  id: 01kzxnjdebmkdbn4pwv5dk7vvs
  text: |-
    Both findings of the 2026-08-13 09:11 round are worked. Each one is now `- [x]`.

    ## The premises hold — nothing was dropped

    Each named line was read before any edit. `"failed to create temp dir: {e}"`
    was at 603, 716 and 719. `"profile-validators"` was at 1173 inside
    `ComponentName::new` and at 1263 as a bare `&str` inside `InitResult::ok`,
    while the two sibling branches wrapped their own. Both premises are true, so
    neither finding was recorded as a conflict.

    ## Finding 1 — the idiom mirdan already has for a shared error message

    mirdan does not name an error message with a `const &str`. It builds the whole
    error in one small private function: `not_found_error(description, global)` in
    `crates/mirdan/src/install/uninstall.rs` is the existing example. The fix
    follows that shape rather than the constant the finding sketched, because a
    constant still leaves each site to spell its own `format!` and its own
    `RegistryError::Validation` wrapper.

    `crates/mirdan/src/install/mod.rs` — the file whose own module doc says it
    "holds the shared path and filesystem helpers" — now holds:

        pub(crate) fn temp_dir_error(e: std::io::Error) -> RegistryError

    The message text is byte-identical to what each site produced.

    The literal was at a FOURTH site the finding did not name:
    `crates/mirdan/src/install/deploy.rs:180`, inside `stage_and_deploy_skill`. A
    helper private to profile.rs would have left that copy behind, and the
    finding's own reason is "so error text changes in one place". All four sites
    now call the helper.

    ## Finding 2 — `ComponentName` at every `InitResult` in the file

    The three repeated component literals each got a constant:
    `PROFILE_SKILLS_COMPONENT`, `PROFILE_AGENTS_COMPONENT` and
    `PROFILE_VALIDATORS_COMPONENT`. This matches the `APPLIER_COMPONENT` constant
    that `install/applier.rs` already declares. Line 1263 is wrapped in
    `ComponentName::new`, as the finding asks.

    The file's own comment block carried the carve-out that let the drift in:
    "The types name what crosses a call boundary; a literal handed straight to
    `InitResult::ok` crosses none." That sentence is wrong, and it is deleted.
    `InitResult::ok(name, message)` takes two adjacent strings, so it IS a
    boundary a caller can cross in the wrong order. The comment now states the
    rule the finding set: every component name this file reports under crosses
    that boundary as a `ComponentName`.
  timestamp: 2026-08-13T13:39:15.531888+00:00
- actor: claude-code
  id: 01kzxnk3kxshsbb2hppkjkyd10
  text: |-
    ## The whole-file sweep, measured

    Finding 2 was a miss from the previous round's sweep, so this round's sweep was
    run with a script rather than by eye. A script over `profile.rs` counts every
    quoted literal outside a comment and groups them.

    ### Cause A — a message literal repeated across call sites: 10 found, 10 fixed

    | literal | sites | fix |
    |---|---|---|
    | `failed to create temp dir: {e}` | 3 in profile.rs + 1 in deploy.rs | `temp_dir_error` helper |
    | `profile-skills` | 2 | `PROFILE_SKILLS_COMPONENT` |
    | `profile-agents` | 2 | `PROFILE_AGENTS_COMPONENT` |
    | `profile-validators` | 2 | `PROFILE_VALIDATORS_COMPONENT` |
    | `skill` | 2 (install label, deinit label) | `SKILL_ITEM_LABEL` |
    | `agent` | 2 (install label, deinit label) | `AGENT_ITEM_LABEL` |
    | `README.md` | 4 (2 `join`, 2 inside a message) | `STORE_README_FILE_NAME` |
    | `Deployed` | 2 reporter verbs | `VERB_DEPLOYED` |
    | `Removed` | 3 reporter verbs | `VERB_REMOVED` |
    | `permissions` / `deny` | 2 each | the `POINTER_KEY_*` constants that already existed |

    The last row is the one worth naming: `POINTER_KEY_PERMISSIONS` and
    `POINTER_KEY_DENY` were declared with the comment "kept in one place so the
    pointer strings and these accessors can never drift", and the `json!` literal
    of `desired_edit_redirect_fragment` restated both. The comment was not true.
    It is now.

    `VERB_INSTALLED` and `VERB_REGISTERED` appear once each. They are declared
    because each is the sibling of a repeated verb in one expression
    (`if install { VERB_INSTALLED } else { VERB_REMOVED }`).

    Three repeated literals stay, and the reason is that the two sites are not one
    concept:

    - `", "` (5 sites) is a join separator, not a message. No two sites must agree.
    - `"command"` (2 sites) is the VALUE of the statusline `type` at one site and
      the KEY of the statusline command at the other.
    - `"agent"` (2 sites) is the reporter label constant at one site and a Liquid
      template variable name at the other.

    After the fix the script reports only those three.

    ### Cause B — a concept that is a newtype at one site and a bare `&str` at another: 3 found, 3 fixed

    The concept is the component name, and the boundary is `InitResult::ok`, which
    takes the component and the message as two adjacent strings.

    1. `InitResult::ok("profile-validators", …)` in `deinit_profile` — the finding.
    2. `InitResult::ok(component, …)` in `apply_profile_settings_fragment`, where
       `component` was `SettingsFragment::component`, a bare `&'static str`.
    3. `InitResult::ok(APPLIER_COMPONENT, …)` in `apply_mcp_operation_at`.

    Two of the three were misses the finding did not name. All six
    `InitResult::ok`/`InitResult::error` calls of the file now pass
    `component.as_str()` from a `ComponentName`.

    ### Where the line is drawn, and why it is not a judgment call

    A parameter that is alone in its type at a call keeps `&str`: `names: &[String]`
    in `run_deinit_step`, `name: &str` in `render_profile_agent`, `set: &str` in
    `stage_validator_set`, `grain: &str` in `report_pruned`. Nothing can be
    exchanged with a lone parameter. This is the same line the previous round drew
    and the reviewer accepted; finding 2 fired precisely because `InitResult::ok`
    is NOT that case.

    A `&'static str` field of a `const` struct also keeps its type:
    `ProfileItemKind::label`, `SettingsFragment::component`, `SettingsFragment::subject`.
    `string_newtype!` wraps `String`, which has no `const` constructor, so a newtype
    there cannot be a `const` at all — the const tables would have to become
    functions or `LazyLock` statics. The constants above give those fields their
    single source of text, which is what the finding asked for; the type cannot
    follow without restructuring the tables. A struct literal must also spell each
    field name, so no value can reach the wrong field.
  timestamp: 2026-08-13T13:39:38.237452+00:00
- actor: claude-code
  id: 01kzxnknk72nzy7rs4cwhb0m4e
  text: |-
    ## The new test, and the proof it is load-bearing

    `InitResult::ok("profile-validators", …)` had no test. Nothing asserted that
    the validator teardown reports under that component name, so the edit could
    have named any component and stayed green. One test was written FIRST, run
    against the unchanged code, and then proved load-bearing by mutation:

    - `deinit_profile_reports_validators_under_the_validators_component` —
      installs one validator set into an explicit root, deinits it, and asserts a
      row named `profile-validators` carrying `Removed 1 validator set(s)`.

    It passed against the unchanged code. The old bare literal at 1263 was then
    mutated to `"profile-skills"`, and the test FAILED:

        validator teardown must report under profile-validators:
        [InitResult { name: "profile-skills", status: Ok, message: "Removed 1 validator set(s)" }]

    The mutation was reverted with `git checkout` before the refactor started.

    The rest of the change is text-identical by construction and already pinned:

    - `init_profile_writes_store_readme_and_deinit_removes_it` pins `README.md` at
      both the write site and the prune site, which is exactly what
      `STORE_README_FILE_NAME` now feeds.
    - `profile_mcp_root_explicit_reports_its_own_verb` pins `Registered` and
      `Removed`, the two `ActionVerb` sites.
    - `deinit_profile_reports_each_family_under_its_own_component_and_kind` pins
      `profile-skills`/`skill` and `profile-agents`/`agent`.
    - `init_profile_installs_edit_redirect_and_deinit_removes` reads the deny array
      back out of a real settings file, so the `json!` key change is covered.

    Every reporter string, every `InitResult` name and message, and every error
    message is byte-identical to before.

    ## Green

    - `cargo nextest run -p mirdan` — 519 passed, 0 failed, 0 skipped (518 before,
      plus the one new test).
    - `cargo nextest run --workspace` — 14179 passed, 0 failed, 0 skipped.
    - `cargo fmt --check` clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` clean.

    Blast radius: `temp_dir_error` is new and `pub(crate)`, so it has no prior
    caller. Every function whose body changed is private to `install::profile`
    except `stage_and_deploy_skill`, whose signature and behaviour are unchanged.
    The workspace run and workspace clippy confirm no dependent moved.

    `git status` lists exactly four files: `install/profile.rs`,
    `install/profile_tests.rs`, `install/deploy.rs` and `install/mod.rs`. The four
    byte-frozen snapshots under `crates/mirdan/retired-validators/` are unmodified,
    and the real `~/.validators/` was never written.
  timestamp: 2026-08-13T13:39:56.647312+00:00
- actor: claude-code
  id: 01kzxnky2pcncc201ymw53tsf5
  text: |-
    ### implement — changed
    - evidence: 4 files — crates/mirdan/src/install/profile.rs, crates/mirdan/src/install/mod.rs, crates/mirdan/src/install/deploy.rs, crates/mirdan/src/install/profile_tests.rs. Both findings of the 09:11 round are `- [x]`. Whole-file sweep: cause A (a message literal repeated across call sites) 10 found / 10 fixed; cause B (a concept that is a newtype at one site and a bare &str at another) 3 found / 3 fixed, 2 of them misses the finding did not name. `cargo nextest run -p mirdan` 519 passed / 0 failed / 0 skipped; `cargo nextest run --workspace` 14179 passed / 0 failed / 0 skipped; fmt and clippy clean.
    - next: /review
  timestamp: 2026-08-13T13:40:05.334060+00:00
- actor: claude-code
  id: 01kzxpngz44sz2h6xrhsx25hpa
  text: |-
    ### review — findings
    - evidence: 4 findings — crates/mirdan/src/install/profile.rs:760, crates/mirdan/src/install/profile.rs:1212, crates/mirdan/src/install/profile.rs:1311, crates/mirdan/src/install/profile_tests.rs:513
    - scope: c9674d01f~1..c9674d01f (round 3). All 5 prior findings checked.
    - dropped: 1 under the existing-tests exception (profile_tests.rs:453, pre-existing test). 0 dropped for a false premise.
    - convergence: the file is churning, not converging. Each round's fix is the only source of the next round's findings. temp_dir_error and all four VERB_* constants were added by c9674d01f itself, and all 4 findings sit on lines this commit wrote. Counts per round: 3, 2, 4 — not falling.
    - next: fix the 4 findings, and remove each cause from the whole file (line 1203 spells the same bare verb as 1212). A human should decide whether string-literal hygiene in profile.rs still belongs to this card, whose stated Done-when is the four fixture snapshots.
  timestamp: 2026-08-13T13:58:26.020112+00:00
- actor: claude-code
  id: 01kzxprkkd29yprmjvqybjpt2h
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 4 files. Both round-2 findings worked. `temp_dir_error` in install/mod.rs follows mirdan's existing idiom (a small private function building the whole error, as `not_found_error` does) rather than a const; a fourth site in deploy.rs that no finding named also calls it. Three component constants replace the scattered literals. A comment in profile.rs had LICENSED the drift — claiming a literal handed straight to `InitResult::ok` crosses no call boundary, which is wrong because that function takes two adjacent strings — and it is deleted.
    - Scripted sweep: 10 repeated literals fixed, 3 newtype inconsistencies fixed, of which the findings named one. Three repeats deliberately kept, because the two sites are not one concept.
    - test: green — 519 mirdan, 14179 workspace, 0 failed, 0 skipped. fmt and clippy clean.
    - commit: c9674d01f
    - review: findings — profile.rs:760, profile.rs:1212, profile.rs:1311, profile_tests.rs:513. 0 dropped for a false premise; 1 dropped under the existing-tests exception.

    **CONVERGENCE JUDGEMENT — the file is churning, not converging.** Recorded because it governs whether this card may continue.

    Counts across rounds: 3, 2, 4. Every one of this round's four findings is a NEW cause that commit c9674d01f itself created — `temp_dir_error` and the four `VERB_*` constants did not exist before it. The loop is mechanical: each fix introduces a new named spelling of a concept, and each new name creates fresh sites where the same concept is still spelled bare. The rule firing is identical every round ("named here, literal there"); only the abstraction it measures against is new. The fix is the finding generator.

    Two further facts:
    1. **Round 2's fix produced a REGRESSION.** Extracting the shared `temp_dir_error` to satisfy a round-2 finding put a factually wrong message on a subdirectory failure at profile.rs:760 — `create_dir_all(&item_dir)` now reports "failed to create temp dir". Obeying the previous round made one site worse. This is a real defect and is fixed first, whatever else happens.
    2. **The author's detector is weaker than the reviewer's.** The round-3 sweep matched whole-literal equality, so it structurally cannot see a repeat that is a TOKEN INSIDE a larger format string — `"Deployed agents to {}"` against `VERB_DEPLOYED`. The next identical sweep would miss the same class again. That asymmetry, not the rule, is why the rounds do not close.

    **Scope note.** The card's own Done-when is the four byte-frozen fixture snapshots plus the guard test, and that has been satisfied since round 1. All three review rounds have been string-literal hygiene in `profile.rs` and its neighbours — files this card never names. What is looping is unrelated work that the reviewed commit range happened to include.

    **Decision for round 4**: fix the regression, fix the three remaining findings, and correct the DETECTOR to compare constant values as substrings so the author's sweep matches the reviewer's before the commit rather than after. If round 5 still produces findings, this card stops and goes to a person, per the finish skill's guardrail.
  timestamp: 2026-08-13T14:00:07.021300+00:00
- actor: claude-code
  id: 01kzxq7z50bcsjzcd5185m478y
  text: |-
    ## Round 4 research — the substring sweep the previous rounds could not run

    Built the detector the round-3 comment said was missing: it extracts every
    `const NAME: &str = "VALUE"` in a file, scans every string literal and format
    string with a Rust-aware tokenizer that skips comments, and matches each const
    VALUE as a **substring**, not by equality.

    Ran on `crates/mirdan/src/install/profile.rs`: 13 consts, **18 raw substring
    sites**. The four findings named 3 of them.

    Also ran it across `profile.rs + mod.rs + deploy.rs + applier.rs` together (57
    sites). That cross-file run is noise and is discarded on a structural ground,
    not a judgment: every const in `profile.rs` is private to that module, so no
    literal in another file can be a second spelling of it. The sweep is scoped to
    the file that declares the const.

    ### The 18 sites, classified

    Genuine — the const's own concept spelled bare (7):

    | line | const | literal |
    |---|---|---|
    | 1203 | VERB_DEPLOYED | `"Deployed skills to {}"` |
    | 1203 | SKILL_ITEM_LABEL | `"Deployed skills to {}"` |
    | 1212 | VERB_DEPLOYED | `"Deployed agents to {}"` (finding 2) |
    | 1212 | AGENT_ITEM_LABEL | `"Deployed agents to {}"` |
    | 1311 | VERB_REMOVED | `"Removed {} validator set(s)"` (finding 3) |
    | 1426 | VERB_REMOVED | `"Removed {} {kind}(s)"` |
    | 513 (profile_tests.rs) | PROFILE_VALIDATORS_COMPONENT | `"profile-validators"` (finding 4) |

    Four of these seven no finding named: 1203 twice, 1212's label, and 1426.
    Line 1426 is the one no round has mentioned at all.

    Collisions — a different concept that happens to contain the same letters (11).
    Substituting the const at any of these would make the code WRONG, so each is
    recorded with its reason:

    - **901, 1155** — `"{verb} applied to {changed} agent(s)"` and
      `"{verb} {subject} for {changed} agent(s)"`. Here `agent` means a *detected
      coding agent* (Claude Code, Cursor). `AGENT_ITEM_LABEL` is documented as "the
      reporter label for one builtin agent". Two different nouns.
    - **280** — `skill_ctx.set("agent", ...)`. A Liquid template variable name, read
      by `_partials/delegate-to-subagent`. Renaming it would break the partials.
    - **289, 455** — `"skill template rendering failed…"`, `"agent template
      rendering failed…"`. English words in a `tracing::warn!`, not the reporter
      label.
    - **400, 430, 617** — `include_str!("../../../../builtin/…/README.md")`.
      Structural, not a choice: `include_str!` takes a literal token and cannot take
      a `const`. These are compile-time SOURCE paths; `STORE_README_FILE_NAME` is
      the name written into the deployed store.
    - **999** — `.expect("fragment permissions.deny is an array")`. Prose describing
      a JSON shape in a panic message for an internal invariant, not a key used for
      lookup. `.expect` takes `&str`; building it from the pointer keys would force
      an allocating `unwrap_or_else(|| panic!(…))`.

    ### Finding 1 is a real regression — premise confirmed at the source

    - `profile.rs:758` is `tempfile::tempdir().map_err(temp_dir_error)` — true.
    - `profile.rs:760` is `create_dir_all(&item_dir).map_err(temp_dir_error)` — the
      temp dir already succeeded one line above, so "failed to create temp dir" is
      factually wrong.
    - `deploy.rs:181` is the same subdirectory failure and says "failed to create
      temp skill dir".
    - `profile.rs:646` and `deploy.rs:179` are real `tempdir()` calls and are correct.

    So the message belongs to two distinct failures at 5 call sites: 3 temp-dir, 2
    subdirectory.

    ### Coverage gaps found

    - `"Deployed skills to {}"` and `"Deployed agents to {}"` have **no test at
      all**. `rg` over every mirdan test file finds no assertion on either.
    - The subdirectory-creation failure at `profile.rs:760` is **unreachable from a
      test**: the function creates its own `tempfile::tempdir()` internally, so no
      test can make the following `create_dir_all` fail. That unreachability is
      exactly why the wrong message survived a round.

    ### Test-file scope

    `c9674d01f` added exactly one test to `profile_tests.rs`
    (`deinit_profile_reports_validators_under_the_validators_component`, lines
    505-520). Finding 4 reaches that test and nothing else in the file; every other
    test there predates the commit and is covered by the existing-tests exception,
    including line 453 which the reviewer already dropped.

    `PROFILE_VALIDATORS_COMPONENT` is currently private to `profile.rs`, so the test
    cannot name it without widening visibility. The precedent is exact:
    `APPLIER_COMPONENT` is `pub(crate)` in `applier.rs` and imported by
    `profile_tests.rs:6`.
  timestamp: 2026-08-13T14:08:30.368291+00:00
- actor: claude-code
  id: 01kzy0689jasvany3rk1esapvc
  text: |-
    ## Round 4 — judgement of the inherited uncommitted work

    A previous agent died mid-run. Its work was read with `git diff` and judged
    before anything was added. Two of the four findings were done, two were not.

    ### What was correct and kept

    - **Finding 1.** `temp_subdir_error(what, e)` in `install/mod.rs` beside
      `temp_dir_error`. `profile.rs` passes `"item"`, `deploy.rs` passes `"skill"`.
      Each site now reports the directory that actually failed, and the repeated
      literal round 2 removed is not back: one function holds the text, and the
      noun is an argument, so no site can report a subdirectory failure without
      naming it. It takes one `&str` plus an `io::Error`, so it carries no
      transposition hazard.
    - **Finding 4.** `profile_tests.rs` names `PROFILE_VALIDATORS_COMPONENT`. The
      three component constants became `pub(crate)`, matching `APPLIER_COMPONENT`.

    ### What was missing

    - **Findings 2 and 3 were NOT done.** `"Deployed agents to {}"` and
      `"Removed {} validator set(s)"` were untouched. The agent had written a
      characterization test for the deploy messages and stopped before the fix.

    ### What did not pin the behaviour — stated plainly

    `test_temp_dir_errors_name_the_directory_that_failed` (install/tests.rs) does
    NOT pin finding 1's fix. It asserts the two helpers' messages, not which helper
    the call site reaches for. Measured, not assumed: reverting `profile.rs` to
    `.map_err(temp_dir_error)` leaves **521/521 green**.

    The failure is unreachable from a test. `stage_and_deploy_rendered` creates its
    own `tempfile::tempdir()` and then `create_dir_all` inside it, so no caller can
    make the second call fail; the item names come from `include_dir!` and are
    fixed. That unreachability is exactly why the wrong message survived round 3.
    The test is kept because it does pin the message contract both sites share, but
    it is not a regression guard for the call site, and it is not presented as one.
  timestamp: 2026-08-13T16:44:51.378511+00:00
- actor: claude-code
  id: 01kzy0701akb6faf0zvn1pk8qr
  text: |-
    ## The substring detector — the thing this round was for

    Rounds 2 and 3 swept with WHOLE-LITERAL EQUALITY. That cannot see a repeat
    that is a token INSIDE a larger format string, which is precisely findings 2
    and 3. The detector is now written and run: it strips comments with a
    Rust-aware walk, collects every `const NAME: &str = "VALUE"`, and matches each
    VALUE as a **substring** of every string literal and format string.
    Cross-file matches count only for `pub`/`pub(crate)` constants, because a
    private constant cannot be a second spelling of a literal in another file.

    ### Counts

    `crates/mirdan/src/install/profile.rs`: **22 sites before, 12 after.**
    `profile_tests.rs`, `deploy.rs`, `mod.rs`, `applier.rs`, `install/tests.rs`:
    **0 sites each.**

    The four findings named **3** of the 22. Seven more were genuine and unnamed:

    | line | constant | literal | named by a finding |
    |---|---|---|---|
    | 1219 | VERB_DEPLOYED | `"Deployed skills to {}"` | no |
    | 1219 | SKILL_ITEM_LABEL | `"Deployed skills to {}"` | no |
    | 1228 | VERB_DEPLOYED | `"Deployed agents to {}"` | **yes (2)** |
    | 1228 | AGENT_ITEM_LABEL | `"Deployed agents to {}"` | no |
    | 1330 | VERB_REMOVED | `"Removed {} validator set(s)"` | **yes (3)** |
    | 1445 | VERB_REMOVED | `"Removed {} {kind}(s)"` | no |
    | 979 | POINTER_KEY_PERMISSIONS | `"/permissions/deny"` | no |
    | 979 | POINTER_KEY_DENY | `"/permissions/deny"` | no |
    | 1006 | POINTER_KEY_PERMISSIONS | `"fragment permissions.deny is an array"` | no |
    | 1006 | POINTER_KEY_DENY | `"fragment permissions.deny is an array"` | no |
    | tests:513 | PROFILE_VALIDATORS_COMPONENT | `"profile-validators"` | **yes (4)** |
    | tests:493-494 | PROFILE_SKILLS/AGENTS_COMPONENT | `"profile-skills"`, `"profile-agents"` | no |

    **No, it did not find nothing beyond the four.** It found 10 more, and every
    one is fixed. Three of them (`979` twice, `1006` twice) were a defect no round
    had seen: `const PERMISSIONS_DENY_POINTER = "/permissions/deny"` restated both
    key constants, while the doc comment above them claimed "kept in one place so
    the pointer strings and these accessors can never drift". That claim was false.
    The pointer is now `permissions_deny_pointer()`, built from the two keys.

    ### The test that separates genuine from collision

    **Must a change to the constant's value reach this literal for the program to
    stay correct?** Yes → genuine. No → the two are different concepts that share
    letters. This is a structural question, not a preference.

    The 12 remaining sites all answer no, and substituting the constant at any of
    them would make the code **wrong**:

    - **104, 107** — `"profile-skills"` contains `"skill"`. The component name is a
      stable reporter row identity; renaming the item label must not rename the row
      users and tests match on. Two independent decisions.
    - **287** — `skill_ctx.set("agent", ...)` is a Liquid variable name read by
      `_partials/delegate-to-subagent`. Substituting would break the partials.
    - **296, 462** — `"skill template rendering failed…"` is English prose in a
      `tracing::warn!`, not a reporter label.
    - **407, 437, 624** — `include_str!("../../../../builtin/skills/README.md")`.
      Structural: `include_str!` takes a literal token and cannot take a `const`.
      These are compile-time repo source paths, not the deployed file name.
    - **908, 1174** — `"{verb} applied to {changed} agent(s)"`. Here `agent` means
      a **detected coding agent** (Claude Code, Cursor). `AGENT_ITEM_LABEL` is the
      reporter label for one **builtin agent**. Two different nouns.

    ### Abstractions: the count

    **0 new names.** Every fix used a constant that already existed
    (`VERB_DEPLOYED`, `VERB_REMOVED`, `SKILL_ITEM_LABEL`, `AGENT_ITEM_LABEL`,
    `PROFILE_*_COMPONENT`, `POINTER_KEY_*`). The one shape change is a rename in
    place: `const PERMISSIONS_DENY_POINTER` became `fn permissions_deny_pointer()`,
    because a `const` cannot join other `const` strings. One name out, one name in,
    10 sites removed. The trade that generated rounds 2 and 3 was not repeated.

    `temp_subdir_error` (1 new name, 2 sites) was inherited from the dead run and
    is finding 1's own prescription, kept for that reason.
  timestamp: 2026-08-13T16:45:15.690620+00:00
- actor: claude-code
  id: 01kzy07sp2x529k61h99zzbphs
  text: |-
    ## Mutation proof — every changed site is pinned, with one stated exception

    Behaviour is byte-identical by design, so the guard is a characterization test.
    Each was run against the unchanged code first, then the constant it feeds was
    mutated to prove the test fails. Every mutation was reverted from a saved copy,
    and the final `rg` confirms the file is back.

    | mutation | result |
    |---|---|
    | `create_dir_all(&item_dir)` back to `temp_dir_error` | **521/521 PASS — not pinned** (see the judgement comment) |
    | `VERB_DEPLOYED` → `"Installed!!"` | FAIL: left `"Installed!! skills to fake-agent"`, right `"Deployed skills to fake-agent"` |
    | `SKILL_ITEM_LABEL` → `"recipe"` + `VERB_REMOVED` → `"Deleted"` | 4 FAIL: `"Deleted 1 recipe(s)"` vs `"Removed 1 skill(s)"`, `"Deployed recipes to fake-agent"`, `"Deleted 1 validator set(s)"`, `"Deleted applied to 1 agent(s)"` |
    | `permissions_deny_pointer` loses its leading `/` | 2 FAIL: `apply_edit_redirect_at_removes_cleanly`, `init_profile_installs_edit_redirect_and_deinit_removes` |
    | `POINTER_KEY_DENY` → `"denied"` | 5 FAIL across `edit_redirect_tests` and `profile_tests` |

    The last two matter most: they prove the derived pointer is covered end to end
    through a real settings file, so no unit test of the helper was needed.

    ### Where the line is drawn in test code, and why it is not a preference

    `init_profile_reports_each_family_deployed_to_its_targets` uses the constant as
    the **lookup key** (`message_of(PROFILE_SKILLS_COMPONENT)`) and the bare literal
    as the **asserted value** (`"Deployed skills to fake-agent"`). Building the
    expected string from `VERB_DEPLOYED` and `SKILL_ITEM_LABEL` would make the
    assertion tautological — it would pass for any verb. The `VERB_*` constants stay
    private to `profile.rs` for exactly that reason, so a test cannot name them.
    The component constants are `pub(crate)` because a lookup key is an identity,
    not the value under test.

    `profile_tests.rs:493-494` were fixed even though a reviewer dropped the same
    lines once under the existing-tests exception. They are not pre-existing test
    code: `git log -L` shows commit `f1601a77d` wrote them, for this card, in round
    2. Finding 4's cause is a test spelling a component name bare where the constant
    exists; removing that cause from the whole file reaches these two lines.

    `init_profile_installs_edit_redirect_and_deinit_removes` keeps
    `settings["permissions"]["deny"]` as bare literals. That is the external Claude
    Code schema being verified, and `POINTER_KEY_*` are private to `profile.rs`,
    so the constants are not visible there and spelling them out is what makes the
    test a real pin.

    ## Green

    - `cargo nextest run -p mirdan` — **521 passed, 0 failed, 0 skipped** (519 at
      round 3, plus 2 new tests).
    - `cargo nextest run --workspace` — **14181 passed, 0 failed, 0 skipped**.
    - `cargo fmt --check` clean. It was NOT clean on the first try — rustfmt
      reflowed the two deploy closures; `cargo fmt` was run and the diff is in the
      change.
    - `cargo clippy --workspace --all-targets -- -D warnings` clean (exit 0).

    Blast radius: `permissions_deny_pointer` is private to `install::profile` and
    `temp_subdir_error` is new, so neither has an outside caller. Every message and
    every error string is byte-identical, proved by the characterization tests.

    `git status` lists exactly five files, all under `crates/mirdan/src/install/`.
    The four byte-frozen snapshots under `crates/mirdan/retired-validators/` are
    unmodified (`git status` on that path returns 0 lines). The real
    `~/.validators/` and the real `$HOME` were never written; all work ran in the
    repo and a scratchpad.

    ## Discovered, out of scope, carded

    `crates/mirdan/src/strategy/mod.rs:35` declares its OWN
    `const PERMISSIONS_DENY_POINTER: &str = "/permissions/deny"`. One external
    contract with two sources in one crate. It is a different module that no finding
    names, and the per-file detector cannot see it, so it is card `^hfx8vw7` rather
    than more churn on this one.
  timestamp: 2026-08-13T16:45:41.954535+00:00
- actor: claude-code
  id: 01kzy083wzqryrqp1gy7xdrbmc
  text: |-
    ### implement — changed
    - evidence: 5 files — crates/mirdan/src/install/profile.rs, crates/mirdan/src/install/profile_tests.rs, crates/mirdan/src/install/mod.rs, crates/mirdan/src/install/deploy.rs, crates/mirdan/src/install/tests.rs. All 4 findings of the 09:41 round are `- [x]`. Substring detector (const VALUE as a SUBSTRING of every literal, not equality): profile.rs 22 sites before / 12 after, all 12 collisions with a structural reason each; profile_tests.rs, deploy.rs, mod.rs, applier.rs, install/tests.rs 0 each. The findings named 3 of the 22; 10 more were genuine and unnamed, all fixed. 0 new abstractions — every fix used an existing constant, and `const PERMISSIONS_DENY_POINTER` became `fn permissions_deny_pointer()` (1 name out, 1 in, 10 sites removed). `cargo nextest run -p mirdan` 521 passed / 0 failed / 0 skipped; `cargo nextest run --workspace` 14181 passed / 0 failed / 0 skipped; fmt and clippy clean.
    - caveat: finding 1's call-site choice is NOT pinned by a test — reverting it leaves 521/521 green, because the failure is unreachable (the function makes its own tempdir and the item names are fixed by `include_dir!`). Stated, not hidden.
    - discovered: `^hfx8vw7` — strategy/mod.rs holds a second source of the permissions.deny pointer.
    - next: /review
  timestamp: 2026-08-13T16:45:52.415464+00:00
- actor: claude-code
  id: 01kzy0rcvxxdjwj9vkcmd1d3ka
  text: |-
    ### review — clean

    - evidence: `review sha 303f7feb7~1..303f7feb7`, 16 validators attempted, 0 failed, 0 skipped, 0 findings. All nine prior findings checked. Task moved to `done`.
    - scope: round 4, the commit only, not the working tree.
    - false premise: 0 findings were dropped for a false premise, because there were no findings.
    - existing-tests exception: not applied this round. No finding was dropped.

    Convergence: this round converged. Rounds 1 to 3 each removed the named lines
    and left the cause, so the next round found new sites in the same file. This
    round removed the cause of the loop instead. The author's sweep now compares
    each const value as a substring of each string literal, so it sees a repeat
    that is a token inside a larger format string. That is the class of repeat the
    review engine reported in every earlier round and the author's equality check
    could not see. The commit also introduced no new abstraction, and a new
    abstraction was what produced fresh sites in rounds 2 and 3.

    An independent substring sweep, written for this review and not the author's
    script, reports 10 remaining sites in profile.rs. Two were read at the named
    line and confirmed structural:

    - `profile.rs:287` — `skill_ctx.set("agent", ...)` is a Liquid template
      variable name that shared partials read as `{{ agent }}`. It is not the
      reporter label. The two share a spelling only.
    - `profile.rs:908` — `"{verb} {subject} for {changed} agent(s)"` counts
      detected coding agents, not builtin agent items. `AGENT_ITEM_LABEL` is the
      label for a builtin item, so this is a different noun.

    The other sites are three `include_str!` path arguments, which the compiler
    requires to be literals, and two tracing sentences where the word is English
    prose. All are false positives of substring matching, not unfixed findings.

    Observation, not a finding, because no validator raised it: the commit message
    states that `test_temp_dir_errors_name_the_directory_that_failed` asserts the
    two helper messages and not which helper a call site reaches for, so reverting
    the call site leaves the suite green. This was read and confirmed at
    `crates/mirdan/src/install/tests.rs`. The test is a message contract, not a
    regression guard for the round-3 fix.

    - next: none. Card closes.
  timestamp: 2026-08-13T16:54:45.885349+00:00
- actor: claude-code
  id: 01kzy0styyjtjemfrtf0ztedsx
  text: |-
    ### finish iteration 4 — clean. The loop closed.
    - Resumed after the round-4 agent died mid-run on a network error, leaving partial uncommitted work. That work was verified, not trusted: two of four findings were done, findings 2 and 3 had not been touched at all, and one inherited test did not pin what it appeared to.
    - implement: changed — 5 files. The regression is fixed properly: `temp_subdir_error(what, e)` takes the noun as a PARAMETER, so a call site cannot report a subdirectory failure without naming the directory, and the repeated literal round 2 removed does not come back.
    - **The detector was the real fix.** Rounds 2 and 3 swept by whole-literal EQUALITY, structurally blind to a repeat sitting as a token inside a format string — which was exactly the class the engine kept reporting. The sweep now compares every const value as a SUBSTRING. It found 22 sites in profile.rs where equality saw 3, fixed 10 that no finding ever named — including `PERMISSIONS_DENY_POINTER` restating both `POINTER_KEY_*` constants under a doc comment claiming they could never drift — and 0 sites in the other five files.
    - **Zero new abstractions**, which was the loop generator in rounds 2 and 3. Every fix used an existing constant; the one shape change is a rename in place.
    - test: green — 521 mirdan, 14181 workspace, 0 failed, 0 skipped. fmt and clippy clean.
    - commit: 303f7feb7
    - review: clean — 16 attempted, 0 failed, 0 skipped, 0 findings. Task moved to done.

    **Convergence confirmed independently.** The reviewer did not accept the 12 documented collisions on the author's word: it wrote its own substring sweep and read two at the line. `skill_ctx.set("agent", …)` is a Liquid variable the shared partials read as `{{ agent }}`; `"{verb} {subject} for {changed} agent(s)"` counts DETECTED CODING AGENTS while `AGENT_ITEM_LABEL` is the reporter label for one BUILTIN agent — a different noun. The rest are `include_str!` path arguments the compiler requires to be literals, and English prose in tracing sentences. All false positives of substring matching.

    Round counts across the card: 3, 2, 4, 0.

    **One thing carried forward, honestly, and it is NOT a finding** — no validator raised it. `test_temp_dir_errors_name_the_directory_that_failed` asserts the two helper MESSAGES, not which helper a call site reaches for; reverting the call-site fix leaves the suite green, because that failure path is close to unreachable. It is a message contract, not a regression guard. The regression fix at profile.rs:764 is held by review, not by a test.

    Also carded rather than swept in: ^hfx8vw7 — crates/mirdan/src/strategy/mod.rs declares its own `PERMISSIONS_DENY_POINTER`, a second source of one external contract, in a module no finding names.
  timestamp: 2026-08-13T16:55:33.086808+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffffffa80
title: Prune the four retired tool-rule fixtures from a deployed validator store
---
`^w6ypb8b` added `RETIRED_VALIDATOR_FILES` to
`crates/mirdan/src/retired_validators.rs`, so a rule file deleted from a
still-shipping builtin set is now pruned out of a deployed store when the user
never touched it.

That card named only the two RULE files. Commit `59bd9ae5c` (card `^wwb6hk7`)
also deleted the four FIXTURE files those rules used:

- `builtin/validators/code-hygiene/fixtures/no-commented-code-parsed.fail.rs.tmpl`
- `builtin/validators/code-hygiene/fixtures/no-commented-code-parsed.pass.rs.tmpl`
- `builtin/validators/duplication/fixtures/duplication-parsed.fail.rs.tmpl`
- `builtin/validators/duplication/fixtures/duplication-parsed.pass.rs.tmpl`

Every store an earlier `sah init` wrote still holds all four. Nothing removes
them, exactly as nothing removed the two rule files.

They raise no `sah doctor` row, because doctor enumerates rules and reads a
fixture only through the rule that names it. No rule names these four any more,
so they are inert bytes. That is why `^w6ypb8b` was complete without them.

## What to build

Add the four fixtures to `RETIRED_VALIDATOR_FILES`. The mechanism already
exists and needs no change: recover each file's exact bytes with
`git show 59bd9ae5c~1:<path>`, write them as byte-frozen snapshots under
`crates/mirdan/retired-validators/<set>/fixtures/`, and add one
`RetiredSetFile` entry for each.

## Done when

- A store holding all four stale fixtures loses all four after `sah init`, and
  a store whose copy of one was edited keeps that one.
- The existing guard tests still pass, in particular
  `test_no_retired_file_is_still_shipped_by_a_builtin_set`.

#tool-validators

## Review Findings (2026-08-13 08:33)

- [x] `crates/mirdan/src/install/profile.rs:647` — Function parameters `name`, `content`, and `file_name` are all `&str` with distinct semantic meanings and should use newtypes to prevent accidental parameter swaps. Define newtypes for semantic clarity: `struct ItemName(String);`, `struct ManifestContent(String);`, `struct ManifestFilename(String);` and update the function signature to use them. This prevents compile-time confusion and makes intent explicit.
- [x] `crates/mirdan/src/install/profile.rs:1027` — Function has three `&str` parameters (`server_name`, `verb`, `preposition`) with distinct semantic meanings and should use newtypes. Define newtypes: `struct ServerName(String);`, `struct ActionVerb(String);`, `struct Preposition(String);` and update the function signature accordingly.
- [x] `crates/mirdan/src/install/profile.rs:1276` — Function parameters `component` and `kind` are both `&str` with distinct semantic meanings and should use newtypes. Define newtypes: `struct ComponentName(String);` and `struct ItemKind(String);` and update the function signature to use them.

Scope of this pass: `99f3229df~1..99f3229df`. The engine raised a fourth finding
on `crates/mirdan/src/install/tests.rs:745` (`make_local_skill`). That helper is
test code that already existed before this commit, so the review skill's
existing-tests exception drops it. It is not an action item.

The engine raised no finding against the four byte-frozen fixture snapshots
under `crates/mirdan/retired-validators/*/fixtures/`. Those files must stay
byte-identical to the git blobs from `59bd9ae5c~1`, or the exact-match prune
stops firing.

## Review Findings (2026-08-13 09:11)

- [x] `crates/mirdan/src/install/profile.rs:603` — Error message 'failed to create temp dir: {e}' appears at lines 603, 716, and 719; extract to a named constant so error text changes in one place. Define `const TEMP_DIR_CREATION_ERROR: &str = "failed to create temp dir";` and use `format!("{}: {e}", TEMP_DIR_CREATION_ERROR)` at all three call sites.
- [x] `crates/mirdan/src/install/profile.rs:1173` — Configuration value 'profile-validators' appears at both line 1173 and line 1263; extract to a named constant so changes propagate to both places. Define `const PROFILE_VALIDATORS_COMPONENT: &str = "profile-validators";` and use it at both call sites; also wrap the line 1263 usage in ComponentName::new() for consistency with skills and agents.

Scope of this pass: `f1601a77d~1..f1601a77d`, a re-review of the three findings
above. All three prior findings are checked. The commit gives the named
functions distinct `string_newtype!` types, so the transposition cause is
removed from the whole file.

Both new findings were verified against the named lines before they were
recorded. The duplicated temp-dir message is present at 603, 716, and 719.
`"profile-validators"` is present at 1173 inside `ComponentName::new` and at
1263 as a bare `&str` inside `InitResult::ok`. No finding was dropped for a
false premise. Neither finding names test code that already existed, so the
existing-tests exception does not apply.

## Review Findings (2026-08-13 09:41)

- [x] `crates/mirdan/src/install/profile.rs:760` — Error message at line 760 was changed to use the generic temp_dir_error helper, which produces 'failed to create temp dir', but this error occurs when creating a subdirectory (item_dir), not the temp directory itself created at line 758. This produces a misleading error message that is inconsistent with the pattern at deploy.rs:182, which uses a specific error message 'failed to create temp skill dir' for the same type of subdirectory-creation error. Use a specific error message for the subdirectory creation, such as `format!("failed to create temp item dir: {e}")` to match the pattern at deploy.rs:182, rather than the generic temp_dir_error which is misleading for this context.
- [x] `crates/mirdan/src/install/profile.rs:1212` — Closure hardcodes 'Deployed' verb in format string instead of using the constant VERB_DEPLOYED. The same verb is used at lines 517 and 624, which both correctly use the VERB_DEPLOYED constant. This is inconsistent; the closure should construct its message using the constant to ensure the verb value is centralized. Change line 1212 from `|targets| format!("Deployed agents to {}", ...)` to `|targets| format!("{} agents to {}", VERB_DEPLOYED, ...)`.
- [x] `crates/mirdan/src/install/profile.rs:1311` — Message hardcodes 'Removed' verb instead of using the constant VERB_REMOVED. Within the same InitResult::ok call (lines 1308-1312), the component name parameter uses ComponentName::new(PROFILE_VALIDATORS_COMPONENT) constant (line 1308, changed in c9674d01), but the message still hardcodes the verb. This is inconsistent; if component names use constants, verbs in messages should too. Change line 1311 from `format!("Removed {} validator set(s)", removed.len())` to `format!("{} {} validator set(s)", VERB_REMOVED, removed.len())`.
- [x] `crates/mirdan/src/install/profile_tests.rs:513` — New test hardcodes the string 'profile-validators' instead of using the constant PROFILE_VALIDATORS_COMPONENT that was added in this same change. Similar tests in the same file use constants (e.g. line 478 uses APPLIER_COMPONENT), establishing a pattern this new test breaks. Change line 513 from `result.name == "profile-validators"` to `result.name == PROFILE_VALIDATORS_COMPONENT`.

Scope of this pass: `c9674d01f~1..c9674d01f`, the third round on this card. All
five prior findings are checked.

Every premise was read at the named line before the finding was recorded. Line
760 does call `temp_dir_error` on `create_dir_all(&item_dir)`, and 758 is the
real `tempfile::tempdir()`; `deploy.rs:181` does say "failed to create temp
skill dir". `VERB_DEPLOYED` is defined at profile.rs:116 and used at 517 and
624, and line 1212 does spell "Deployed" as a bare literal. `VERB_REMOVED` is
defined at 119, and line 1311 does spell "Removed" as a bare literal.
Line 513 does compare against the bare literal `"profile-validators"`. No
finding was dropped for a false premise.

One finding was dropped under the review skill's existing-tests exception: the
engine also named `crates/mirdan/src/install/profile_tests.rs:453`
(`message_of("profile-skills")`). That line is not in this commit's diff, so it
is test code that already existed, and the finding asks only to restyle it. It
is not an action item. The finding at line 513 is NOT dropped, because that
test is new in this commit.

Line 1212 is one example of its cause, not the whole of it. Line 1203 spells
the same verb as a bare literal in `format!("Deployed skills to {}", ...)`.
Remove the cause from the whole file, not only the named line.

Origin of this round: `temp_dir_error` and all four `VERB_*` constants were
added by this same commit, `c9674d01f`. Each of these four findings is on a
line this commit itself wrote or changed. They are not the round-2 findings
left unfixed.