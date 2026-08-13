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
position_column: doing
position_ordinal: '8280'
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