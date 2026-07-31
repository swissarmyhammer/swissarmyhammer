---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kywek7d4ew3myfv0eawdwxhp
  text: |-
    Picked up by /finish ^qsr5rdt. No prior attempts.

    State at pickup, HEAD aecbd1216:
    - All 8 `profiles:` keys still present — code-context, explore, finish, implement, issue, kanban, plan, review.
    - `task` already lost its key in the user's commit b6cfd88cb, which is what surfaced this whole thread: two kanban-app deploy tests went red because they hardcode `task` in the expected kanban-profile roster.
    - Four `builtin/_partials/*.md` deletions are UNCOMMITTED in the working tree (coding-standards, short-ids, skills, validators) — the user's parallel dedup work. Leave them alone; do not stage them, do not restore them.

    Design decision came from the user directly: "everything deploys everywhere". `Selector::All` for every consumer, curation removed. Kanban.app and kanban-cli will ship all 23 skills including /ci, /coverage, /make-readme, /deduplicate. That is the accepted outcome, not an oversight — do not reintroduce filtering under another name.

    Worth knowing before starting: deleting `Skill::profiles` also deletes `SAH_INTERNAL_FRONTMATTER_KEYS`, which I added two hours ago on ^t7ebyn8 for the sole purpose of keeping `profiles` out of deployed SKILL.md files. With `profiles` gone there is no internal key left, so the constant, its filter in `format_skill_md`, the `extra.retain` in the loader, and the three tests that loop over it all go. Verify no other key is genuinely internal before deleting rather than assuming — the loader/deploy set difference was exactly `{profiles}` when I checked.

    The lossless round-trip for the 12 unmodeled Claude Code keys (hooks, model, paths, etc.) must keep working after that removal. That is the point of ^t7ebyn8 and the reason the ralph Stop hook now survives deployment.
  timestamp: 2026-07-31T16:02:25.828863+00:00
- actor: claude-code
  id: 01kywj2vy200b5zdsya6s24ptc
  text: |-
    Implementation landed. Worked from HEAD aecbd1216.

    ## TDD order

    Flipped the two kanban-app deploy tests FIRST, with production still on `Selector::Profile("kanban")`, and watched them go RED for the right reason:
    - `workspace_init::opening_a_board_deploys_the_kanban_tool_skills` panicked on "builtin skill `map` must be deployed".
    - `state::tests::test_open_board_deploys_kanban_tool_skills_at_board_folder` panicked on "builtin skill `detected-projects` must be deployed".

    Both now GREEN.

    ## Call sites → Selector::All

    - `apps/kanban-app/src/state.rs::kanban_profile`
    - `apps/kanban-cli/src/commands/registry.rs::profile` (plus the `KANBAN_PROFILE` const, deleted)
    - `apps/code-context-cli/src/commands/registry.rs::skills_selector` (plus the `SKILL_NAMES` const, deleted) — reused unchanged by `commands/skill.rs`
    - `apps/kanban-app/tests/workspace_init.rs::kanban_profile` (test-local mirror)
    - The two reconstructed profiles in `mirdan::install::profile_consistency_tests` (`kanban_profile`, `code_context_profile`)

    ## Mechanism deleted

    - `Selector::Profile` variant + its arm in `Selector::select`
    - `KNOWN_PROFILES` + the `debug_assert!` that validated builtins against it
    - `resolve_profile_skills` (it had ZERO callers — dead since it was written)
    - `Skill::profiles`, plus the `profiles` field on both `SkillFrontmatter` structs (loader + deploy)
    - `SAH_INTERNAL_FRONTMATTER_KEYS`, its `format_skill_md` filter, the loader's `extra.retain`, and the three tests that looped over it
    - Two obsolete loader tests: `test_parse_skill_md_parses_profiles_list`, `test_parse_skill_md_profiles_default_empty_when_absent`

    `Selector::select` also changed shape: it took a `HashMap<String, Vec<String>>` name → profile-tags map purely so `Profile` could match tags. Six call sites built that map, four of them filling `Vec::new()` placeholders with "carries no profile tags" comments. It now takes `&HashSet<String>` of names. Leaving the tag map would have been dead plumbing and a profile-shaped vestige.

    ## No other key was internal

    Verified before deleting the constant: the loader's `SkillFrontmatter` names `name, description, license, compatibility, context, agent, metadata, allowed-tools, profiles`; the deploy `SkillFrontmatter` names the same set minus `profiles`. The difference was exactly `{profiles}`, so the internal set is now empty and the constant is gone. **No other key turned out internal.**

    ## ^t7ebyn8 not regressed

    `deploy.rs::test_format_skill_md_round_trips_unmodeled_frontmatter_keys` (the lossless round-trip over `hooks`/`model`/`paths`/`disable-model-invocation`) is untouched and passes, as is `test_format_skill_md_writes_each_modeled_key_once` (I only dropped the now-unmodeled `profiles:` from its fixture). `mirdan::install::init_profile_preserves_unmodeled_skill_frontmatter` got STRONGER: with no internal keys there is nothing to skip, so every source key must now equal the deployed value except Liquid-rendered `metadata`.

    Verified end to end in a sandbox (`HOME` + `XDG_CONFIG_HOME` redirected to a temp dir, real `~/.skills` mtime confirmed older than the run): the deployed `finish/SKILL.md` still carries its `hooks: Stop: command: sah tool ralph ralph check --` block, and no deployed SKILL.md carries a `profiles:` key.

    ## 6 → 23 proof

    Measured the before-count mechanically at HEAD: 6 builtins carried `- kanban` (finish, implement, issue, kanban, plan, review) and 2 carried `- code-context` (code-context, explore). After: sandboxed `kanban init project` logs "Deployed 23 skill(s) to claude-code" and `<root>/.skills` holds all 23. Sandboxed `sah init project` also lands 23.

    ## Interesting discovery — orphan file

    `crates/mirdan/src/plugin.rs` is NOT compiled. `crates/mirdan/src/lib.rs` never declares `mod plugin`, nothing references `mirdan::plugin`, and there is no `#[path]` attribute or extra bin target pulling it in. It carried a `Selector::Profile("kanban")` plugin spec and its own tag-map plumbing. I updated it to `Selector::All` + a name set so no stale `Selector::Profile` text survives anywhere, but the file is dead code that the compiler never checks — so it silently rots. Filed separately; it wants either wiring up or deleting.

    ## What did not work / notes

    - `cargo nextest run -p kanban-app` appears to hang: `kanban-app::ai_panel_e2e test_ai_panel_e2e_mcp_tool_reachable_in_session` is a REAL-MODEL qwen-0.6b test and ran past 9 minutes. 158/160 kanban-app tests pass, including both target tests; the two stragglers are the ai_panel_e2e pair. Not caused by this card — exclude `binary(ai_panel_e2e)` and the `llama-agent`/`claude-agent` packages when running the suite.
    - Added `swissarmyhammer-skills` to `apps/kanban-app` `[dev-dependencies]`. The reassertion reads the real `SkillResolver::new().resolve_builtins()` roster instead of transcribing 23 names, so the test cannot drift as builtins are added or removed. kanban-app had no skills dependency before, only `mirdan`, which does not re-export the resolver.
  timestamp: 2026-07-31T17:03:24.098100+00:00
- actor: claude-code
  id: 01kywkye0st63fcjc4zvrds7j4
  text: |-
    Verification complete. Green, with two pre-existing exceptions that are not this card's.

    ## Acceptance sweep (0 hits each, excluding `target/`)

    - `Selector::Profile` — 0
    - `KNOWN_PROFILES` — 0
    - `resolve_profile_skills` — 0
    - `SAH_INTERNAL_FRONTMATTER_KEYS` — 0
    - `Skill::profiles` / `profiles: Vec<String>` / `profiles: fm.profiles` — 0
    - `^profiles:` anywhere under `builtin/` — 0

    ## Sandboxed init proof

    Sandbox at a `mktemp -d`, with `HOME` and `XDG_CONFIG_HOME` redirected into it. The real `~/.skills` mtime stayed at 11:16, well before the 12:01 runs, and `git status -- .skills .claude` stayed empty — nothing wrote to the real home or to this repo.

    - `sah init project` → `<root>/.skills` holds 23 skills: check-sah ci code-context commit coverage deduplicate detected-projects double-check explore finish implement issue kanban lsp make-readme map plan review shell task tdd test thoughtful
    - `kanban init project` → logs `+ Deployed 23 skill(s) to claude-code`; `<root>/.skills` holds the same 23
    - No deployed `SKILL.md` in either sandbox carries a `profiles:` key
    - `finish/SKILL.md` still carries its `hooks:` block — `Stop: - hooks: - type: command, command: sah tool ralph ralph check --` — so ^t7ebyn8 is intact through a real init

    ## 6 → 23

    Before-count measured mechanically against HEAD aecbd1216: 6 builtins carried `- kanban` (finish, implement, issue, kanban, plan, review), 2 carried `- code-context` (code-context, explore). After: 23 for every consumer.

    ## Test / fmt / clippy

    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, clean
    - `cargo fmt --check` — exit 0, clean
    - `cargo nextest run --workspace --no-fail-fast --exclude llama-agent --exclude claude-agent -E 'not (binary(ai_panel_e2e))'` — **12578 tests run: 12574 passed (41 slow), 3 failed, 1 timed out**

    The 4 non-passes are all pre-existing and unrelated:

    1. **3 failed** — `swissarmyhammer-templating::skills_rendering_test` (`test_skills_partial_hidden_when_no_skills`, `test_skills_partial_renders_with_available_skills`, `test_agent_system_prompt_includes_skills_section`), all panicking `Prompt '_partials/skills' not found`. Caused by the deletion of `builtin/_partials/skills.md` — the parallel dedup work this card was told not to touch. I did not restore it and did not delete the tests; the call is whether an agent system prompt should still advertise skills. Filed as ^anwbdbz.
    2. **1 timed out** — `swissarmyhammer-agent::review_real_model_e2e review_runs_over_acp_against_a_real_local_model`, TMT at 480s on TRY 3. Real-model test, the known category that blows timeouts.

    Also excluded up front and reported rather than waited on: `kanban-app::ai_panel_e2e` (real-model qwen-0.6b; ran past 9 minutes standalone) and the `llama-agent`/`claude-agent` packages. Running kanban-app on its own gave 158/160 with only the two ai_panel_e2e tests outstanding — both target deploy tests passed. The known flake `review_working_emits_progress_notifications_per_pair_when_token_supplied` (^t681xdv) passed this run.

    ## Note on git state

    Between my last local verification and this comment the user committed in a parallel shell: my changes are now in `ed28b4d1d refactor(skills): remove profile-based skill selection`, bundled together with the `builtin/_partials/*` deletions. I did not stage or commit anything myself. HEAD moved from aecbd1216 → ed28b4d1d. Card left in `doing`.
  timestamp: 2026-07-31T17:35:55.929370+00:00
- actor: claude-code
  id: 01kywq62sdeb431dn8r80f89pw
  text: |-
    Closing. 61 findings, **zero on code this commit introduced**, 54 split out, 7 dropped under policy.

    The reviewer verified classification against post-image hunk ranges rather than trusting citations, and everything this commit added — doc comments, `skills: Some(Selector::All)`, the `Selector::select(&HashSet<String>)` signature and its three call-site builders, the strengthened kanban-app test — drew no finding at all. Notably nothing on the rewritten `select`.

    Split, folded into existing cards where they already existed rather than duplicating:
    - install.rs cluster (14) → appended to ^927239f, which already covered PackageType dispatch and constants. Second independent round landing on that file.
    - skill_loader.rs / skill.rs cluster (8) → appended to ^s7hpzhg. Also recorded that `SAH_INTERNAL_FRONTMATTER_KEYS` no longer exists, so part of that card's scope is now moot.
    - lib.rs doc sweep (18) → ^t1n2b7q
    - state.rs production literals (4) → ^yj9n6ke
    - resolver convergence → ^m4eb4bf

    Dropped, not recorded: 2 on the orphan `plugin.rs` (deleted, ^df28f8k), and 5 under the test-refactor exception — `mod tests` starts at `state.rs:1391`, so the magic 25/20, `worker_threads = 2` and `from_secs(2)` items are all test code.

    The engine's line numbers were worse than "frequently offset" on this commit, and this is worth carrying forward: `state.rs:646` was cited for an FNV seed that lives at ~1312, where 646 is `pub fn new_for_test()`. `install.rs:1441` was cited for metadata rendering; 1441 is `fn resolve_agent_file`. Offsets ran 200+ lines. Because this commit was largely deletions, code that merely moved up got reported as new work — `skill.rs:18` for `SkillName::new` is the clearest case, displaced from ~37 by the 19 deleted lines above it, signature untouched.

    Shipped:
    - Profile-based skill selection is gone. `Selector::Profile`, `KNOWN_PROFILES`, the validating `debug_assert!`, `Skill::profiles`, `resolve_profile_skills` (zero callers), `SAH_INTERNAL_FRONTMATTER_KEYS` and its filter, and the `profiles:` key in 8 SKILL.md files.
    - `Selector::select` now takes `&HashSet<String>` instead of a `HashMap<String, Vec<String>>` that existed only for profile matching — 4 of its 6 call sites were passing `Vec::new()` placeholders.
    - `KANBAN_PROFILE` and `SKILL_NAMES` constants deleted; every consumer uses `Selector::All`.
    - The kanban-app deploy test now reads the real resolver roster instead of transcribing 23 names, so it cannot drift.
    - 6 → 23 proved in a sandbox for both `sah init project` and `kanban init project`, with HOME/XDG_CONFIG_HOME redirected. Real `~/.skills` mtime unchanged.

    The risk on this card was that deleting `SAH_INTERNAL_FRONTMATTER_KEYS` would break the frontmatter round-trip that makes the ralph Stop hook survive deployment (^t7ebyn8). It does not: the sandbox-deployed `finish/SKILL.md` still carries its `hooks: Stop:` block, no deployed file carries `profiles:`, and `init_profile_preserves_unmodeled_skill_frontmatter` is now stronger — with no internal keys there is nothing left to skip.

    Two follow-ups from this card's own work, both resolved by user decision and already closed: ^anwbdbz (the `_partials/skills.md` tests, deleted) and ^df28f8k (the orphan `plugin.rs`, deleted — it was never compiled, which is why it still held `Selector::Profile` with clippy silent at exit 0).
  timestamp: 2026-07-31T18:32:32.301111+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8580
title: 'Remove profile-based skill selection: every consumer deploys all builtin skills'
---
Delete the init-profile mechanism for skills. Every consumer deploys all 23 builtin skills via `Selector::All`. Curation by profile goes away entirely — this is a deliberate product decision, not a simplification of an existing behavior.

## Today

| Consumer | Selector | Skills |
|---|---|---|
| `sah` | `All` | 23 |
| `kanban-cli`, `Kanban.app` | `Profile("kanban")` | 6 — finish, implement, issue, kanban, plan, review |
| `code-context-cli` | `Profile("code-context")` | 2 — code-context, explore |
| `shelltool-cli` | `Single("shell")` | 1 |

After: `Kanban.app` and `kanban-cli` ship `/ci`, `/coverage`, `/make-readme`, `/deduplicate` and the rest. That is the accepted outcome.

## Required change

1. **Call sites → `Selector::All`**
   - `apps/kanban-app/src/state.rs:1156`
   - `apps/kanban-cli/src/commands/registry.rs` — three sites, around lines 40, 84, 97
   - `apps/code-context-cli/src/commands/registry.rs::skills_selector` (and its reuse in `commands/skill.rs:30`)

2. **Delete the mechanism**
   - `Selector::Profile` variant and its arm in `Selector::select` (`crates/mirdan/src/install.rs:886`)
   - `KNOWN_PROFILES` (`install.rs:1048`) and the `debug_assert!` validating against it (~1105-1119)
   - `resolve_profile_skills` (`crates/swissarmyhammer-skills/src/deploy.rs`)
   - `Skill::profiles` field and the `profiles` field on both `SkillFrontmatter` structs (loader + deploy)

3. **`SAH_INTERNAL_FRONTMATTER_KEYS` becomes empty — delete it too.** It exists solely to keep `profiles` out of deployed SKILL.md. With `profiles` gone there is no internal key left, so the constant, its filter in `format_skill_md`, the `extra.retain` in the loader, and the three tests that loop over it all go. Check for a genuinely-internal key before deleting; if one turns up, keep the constant and say which.

4. **Strip `profiles:` from the 8 builtin SKILL.md files** that still declare it: code-context, explore, finish, implement, issue, kanban, plan, review. (`task` already lost its key.)

5. **Fix the two kanban-app deploy tests** that hardcode the kanban-profile roster — `workspace_init::opening_a_board_deploys_the_kanban_tool_skills` and `state::tests::test_open_board_deploys_kanban_tool_skills_at_board_folder`. They currently fail because `task` lost its `profiles:` key. Reassert against the full builtin set.

## Coordination

The user is editing `builtin/skills/**` in parallel to reduce duplication. Item 4 touches only the `profiles:` frontmatter key in 8 files — coordinate before editing, and re-read each file immediately before changing it.

## Acceptance

- No `Selector::Profile`, `KNOWN_PROFILES`, `resolve_profile_skills`, or `Skill::profiles` anywhere.
- No `profiles:` key in any `builtin/skills/*/SKILL.md`.
- A sandboxed `sah init project` (redirected HOME, never the real `~/.skills`) deploys all 23 skills.
- The same for a kanban-cli init — prove the count went 6 → 23.
- `cargo nextest run --workspace`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Related: ^t7ebyn8 added the frontmatter round-trip that `SAH_INTERNAL_FRONTMATTER_KEYS` guards; deleting the constant must not break the lossless round-trip for the 12 unmodeled Claude Code keys. #refactor #init #bug

## Review Findings (2026-07-31 12:37)

Scope: `ed28b4d1d^..ed28b4d1d`. Engine counts: 61 findings / 61 confirmed / 34 refuted.

**Classification: 0 findings land on code this commit introduced.** Everything this commit added is doc comments, `Selector::All`, the `HashSet<String>` selector signature, and a strengthened kanban-app test that reads the real resolver roster. The engine raised no finding on any of those lines.

7 findings were dropped and are not listed: 2 on the orphan `crates/mirdan/src/plugin.rs` (never compiled, tracked by ^df28f8k), and 5 asking to refactor pre-existing test code (the review test-refactor exception) — the `for i in 0..25` / `boards.len(), 20` magic numbers at `state.rs:1458,1463`, `worker_threads = 2` at `state.rs:1852`, the `from_secs(2)` probe timeout at `state.rs:1963`, and the `create_board_at` helper dedup at `workspace_init.rs:48`.

The 54 items below are all on **pre-existing** code — either untouched by this commit, or merely displaced upward when a deleted block vanished. The engine's cited line numbers track the pre-image and are frequently offset; verified real locations are noted where they differ. Every item is a split candidate; none blocks this card's acceptance criteria.

- [ ] [PRE-EXISTING] `apps/code-context-cli/src/commands/skill.rs:18` — The Profile constructed in run_skill() is a near-duplicate of the profile() function in registry.rs, differing only in mcp_server: None vs Some(...) — this is one function with two parameter variants. Extend the registry profile() function to accept a boolean or enum parameter controlling whether to include mcp_server, so both callers (registry and skill.rs) use the same builder. (Verified: this commit changed only the module doc comment in this file; the `Profile` construction is untouched.)
- [ ] [PRE-EXISTING] `apps/kanban-app/src/state.rs:222` — Function `register_view_store()` reimplements the pattern from `register_perspective_store()` (line ~202) with only type and context-access differences. The doc comment explicitly acknowledges this ('Mirrors [`register_perspective_store`]'), yet the identical boilerplate pattern could be unified. Extract the common registration pattern into a generic or trait-based helper function that abstracts over store type and context access, eliminating the acknowledged duplication.
- [ ] [PRE-EXISTING] `apps/kanban-app/src/state.rs:471` — The pub(crate) method `set_text` lacks a documentation comment, while the adjacent method `set_enabled` has one. This creates inconsistency in the public API. Add a documentation comment to `set_text` explaining its purpose, e.g. `/// Set the text of this menu item.`. (Real location: `set_text` at line 571.)
- [ ] [PRE-EXISTING] `apps/kanban-app/src/state.rs:480` — The pub(crate) field `boards` lacks documentation while all other pub(crate) fields in AppState have documentation comments, creating an inconsistency in the struct's public API. Add a documentation comment to the `boards` field, e.g. `/// Map of open board paths to their handles, keyed by canonical path.`.
- [ ] [PRE-EXISTING] `apps/kanban-app/src/state.rs:646` — Hardcoded FNV-1a hash seed `5381u64` is unexplained — this is a well-known hash constant but should be named for clarity and maintainability. Define `const FNV_OFFSET_BASIS: u64 = 5381u64;` and use that name instead. (Real location: line 1312, production code, untouched by this commit.)
- [ ] [PRE-EXISTING] `apps/kanban-app/src/state.rs:704` — Hardcoded JPEG magic byte `0xFF` is unexplained — this is the first byte of the JPEG SOI (Start of Image) marker but should be named for clarity. Define `const JPEG_SOI_BYTE_1: u8 = 0xFF;` and use that instead. (Real location: line 1377, production code, untouched.)
- [ ] [PRE-EXISTING] `apps/kanban-app/src/state.rs:704` — Hardcoded JPEG magic byte `0xD8` is unexplained — this is the second byte of the JPEG SOI (Start of Image) marker but should be named for clarity. Define `const JPEG_SOI_BYTE_2: u8 = 0xD8;` and use that instead. (Real location: line 1377, production code, untouched.)
- [ ] [PRE-EXISTING] `apps/kanban-app/src/state.rs:809` — The error-handling block for extracting board_dir from kanban_path.parent() is repeated verbatim at line 883 in start_board_mcp_server, differing only in the warning message and return type — should be extracted to a parameterized helper. Extract a helper function parameterized on the warning message and return type, or use a Result-based pattern that both callers can leverage to avoid the copy.
- [ ] [PRE-EXISTING] `apps/kanban-app/src/state.rs:883` — The error-handling block is a near-verbatim copy of the same pattern at line 809 in ensure_workspace_tools, differing only in warning message and return type. Extract a shared helper function parameterized on the context message and return strategy to eliminate this duplication.
- [ ] [PRE-EXISTING] `apps/kanban-cli/src/commands/registry.rs:22` — Function `profile()` is functionally identical to `apps/code-context-cli/src/commands/registry.rs:profile()` (line ~24). The only structural difference is that code-context indirects through `skills_selector()` which returns `Selector::All`, while kanban inlines it—both are equivalent. Extract a shared profile template or helper function in a common module to eliminate duplication across both CLI tools. (The `Profile { .. }` literal shape was already identical before this commit; only the `skills:` field expression changed. Cross-crate refactor.)
- [ ] [PRE-EXISTING] `apps/kanban-cli/src/commands/registry.rs:33` — profile() function is nearly identical to the profile() in code-context-cli/registry.rs — both construct Profile with identical structure differing only in SERVER_NAME. Extract a shared profile builder function parameterized on SERVER_NAME so both registries use the same implementation.
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:90` — PackageType dispatch match repeated identically at lines 90, 167, 362. Same 5-arm match over PackageType calling deploy_skill/validator/tool/plugin/agent in the same order should be a single dispatch function, not repeated control flow. Extract a single `dispatch_deploy_package(pkg_type: PackageType, name: &str, source_dir: &Path, agent_filter: Option<&str>, global: bool) -> impl Future<Output = Result<Vec<String>>>` function that handles all 5 variants, called from run_install_local, run_install_git, and install_from_archive. (This commit touched only lines 851+ in this file.)
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:98` — PackageType dispatch for manifest file path selection repeated at lines 98 and 211. Both match on package_type to determine which manifest file (.SKILL.md, VALIDATOR.md, TOOL.md, AGENT.md) to read frontmatter from—should be a single lookup table, not repeated match statements. Create a lookup table `const MANIFEST_FILE: &[(PackageType, &str)] = &[(PackageType::Skill, "SKILL.md"), (PackageType::Validator, "VALIDATOR.md"), ...]` and use it to look up the manifest filename once, then call `read_frontmatter(&dir.join(filename))` instead of repeating the match.
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:167` — PackageType dispatch match (second occurrence of pattern at line 90). Identical structure repeating control flow that should be data-driven. See line 90 finding—consolidate all three matches into a single dispatch function.
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:169` — Function parameter takes concrete `PathBuf` instead of accepting a generic type — violates 'accept generics, not concrete types' principle. Change signature to accept `impl AsRef<Path>`: `fn rooted(root: Option<&Path>, global: bool, path: impl AsRef<Path>) -> PathBuf`, then update match arms to call `.as_ref()` or `.to_path_buf()` as needed.
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:211` — PackageType dispatch for manifest file path selection (second occurrence of pattern at line 98). Same match structure appears within the git install loop to determine which manifest file to read. See line 98 finding—consolidate manifest file selection into a single lookup table and reuse it at both lines 98 and 211.
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:362` — PackageType dispatch match (third occurrence of pattern at line 90). Identical parallel control flow repeating the same dispatch logic. See line 90 finding—consolidate all three matches into a single dispatch function.
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:1322` — 5 levels of nested conditionals checking lockfile state across HOME and CWD. The nested if/if-let statements (lines 1322–1326) create a difficult-to-follow code path with multiple exit conditions that are hard to reason about together. Extract the HOME fallback logic into a separate helper function, e.g., `fn load_fallback_lockfile(project_root: &Path, home: Option<&Path>) -> Result<Lockfile, RegistryError>`. This isolates the nested conditionals and makes the primary function flow clearer. Alternatively, use early returns or the `?` operator to flatten the nesting. (Lockfile loading is unrelated to profile selection; untouched.)
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:1441` — Metadata template rendering (clone object, iterate values, render if containing template markers) duplicates logic already in `render_profile_skill` at lines 1250–1256. Both perform the identical for-loop over `metadata.values_mut()`, checking for `{{` or `{%` and calling `library.render_text()` on matching values. Extract the metadata rendering loop into a shared helper function: `fn render_metadata_values(metadata: &mut HashMap<String, String>, library: &TemplateLibrary, ctx: &TemplateContext)` and call it from both `render_profile_skill` and the loop in `install_profile_agents`. (Verified: both loops pre-date this commit; neither was added here.)
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:1533` — 4+ levels of nesting with a complex boolean OR condition inside nested loops and conditionals. The condition checks two alternatives with `.as_deref() == Some(name)`, creating cognitive load. The nested while loop immediately after (line 1537) creates 5-6 levels of nesting in the worst case. Extract the directory matching logic into a helper function (e.g., `fn matches_target_skill(fm_name: Option<String>, dir_name: Option<String>, target: &str) -> bool`). Extract the empty-directory cleanup into a separate function to reduce the nesting depth and complexity of this recursive helper.
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:1625` — `resolved_agent_names` reimplements the structure of `resolved_skill_names` (line 1618) exactly: create resolver, resolve builtins, extract keys to HashSet, call selector.select(). The only difference is the resolver type (`AgentResolver` vs `SkillResolver`), making them near-duplicates. Unify with a generic or trait-based approach: define a trait `BuiltinResolver` that both `SkillResolver` and `AgentResolver` implement, then create a single generic `fn resolved_builtin_names<R: BuiltinResolver>(selector: &Selector) -> Vec<String>`, or parameterize the resolver as an argument. (Real location: lines 2151-2166. NOTE — the strongest "introduced" candidate: both functions were already parallel before this commit, but they built different maps (skills used real profile tags, agents an empty placeholder); collapsing both to `HashSet<String>` made them textually identical. The parallel structure is pre-existing; the convergence is a consequence of this commit. Unifying two resolvers that live in different crates behind a new trait is its own card.)
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:1705` — `read_skill_frontmatter_name` reimplements YAML frontmatter parsing (strip `---`, find closing marker, parse YAML, extract name field) that `read_frontmatter` already provides. The duplicated core logic at lines 1707–1717 mirrors lines 273–292 of the existing function. Replace `read_skill_frontmatter_name` with a call to `read_frontmatter(path).map(|(name, _)| name).ok()`, or define `read_skill_frontmatter_name(path)` as `read_frontmatter(path).ok().map(|(n, _)| n)` to avoid duplicating the frontmatter parsing.
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:1794` — Excessive nesting depth (4+ levels starting here, escalating to 8-10 levels by line 1809) with multiple sequential if-let statements. The function contains a deeply nested if-let chain: line 1794 (4 levels) → line 1799 (5 levels) → line 1803 (6 levels) → line 1804 (7 levels) → line 1805 (8 levels) → line 1809 (10 levels with nested for loop). This pattern makes the code difficult to understand and maintain. Extract the MCP server registration logic into a separate helper function, e.g., `fn register_plugin_mcp_servers(agent: &AgentDef, plugin_mcp: &Path, mcp_cfg: &agents::McpConfigDef, config_path: &Path) -> Result<(), RegistryError>`. This reduces the nesting inside `deploy_plugin` and improves readability. The check for `plugin_mcp.exists()` and subsequent registration should be refactored into one or more smaller, focused functions.
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:1820` — Skills deinit reports `names.len()` (count of names to remove) even if some uninstalls fail with errors, but validators deinit only reports `removed.len()` (count actually removed). The three code paths should be consistent: either all report attempted vs actual removals the same way, or only validators should track actual removals. Make skills and agents deinit track actual successes like validators do: collect removal results and only include successfully removed items in the count, or change the message to 'Attempted to remove' if trying all regardless of outcome is intentional. Currently the message 'Removed X' is misleading when some removals encounter errors. (Real location: line 2045; untouched by this commit.)
- [ ] [PRE-EXISTING] `crates/mirdan/src/install.rs:1840` — Agents deinit has the same inconsistency as skills deinit: reports `names.len()` (attempted removals) rather than actual successful removals, diverging from the validators deinit pattern. Align agents deinit with validators deinit: track how many agents were successfully uninstalled and report only that count, not the count of attempted removals. (Real location: line 2067; untouched.)
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/deploy.rs:67` — The "---" YAML frontmatter delimiter is hardcoded in the format_skill_md function's format string. This same delimiter is already hardcoded in skill_loader.rs split_frontmatter, creating cross-file duplication that should be unified in a shared constant. Extract a shared module constant for the YAML delimiter (e.g., const SKILL_MD_FRONTMATTER_DELIM: &str = "---") in a shared location (e.g., a constants module) and use it in both deploy.rs format_skill_md and skill_loader.rs split_frontmatter to avoid duplication and ensure consistency across parsing and formatting. (Verified: this commit's only change to `format_skill_md` replaced the filtered `extra` with `skill.extra.clone()`; the delimiter in the format string is untouched.)
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:19` — Public module `context` declaration lacks a doc comment. All public items must have documentation. Add a doc comment: `/// [Description of the context module]` before the module declaration.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:20` — Public module `deploy` declaration lacks a doc comment. All public items must have documentation. Add a doc comment describing the deploy module.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:21` — Public module `error` declaration lacks a doc comment. All public items must have documentation. Add a doc comment describing the error module.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:22` — Public module `operations` declaration lacks a doc comment. All public items must have documentation. Add a doc comment describing the operations module.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:23` — Public module `parse` declaration lacks a doc comment. All public items must have documentation. Add a doc comment describing the parse module.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:24` — Public module `schema` declaration lacks a doc comment. All public items must have documentation. Add a doc comment describing the schema module.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:26` — Public module `skill_library` declaration lacks a doc comment. All public items must have documentation. Add a doc comment describing the skill_library module.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:27` — Public module `skill_loader` declaration lacks a doc comment. All public items must have documentation. Add a doc comment describing the skill_loader module.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:28` — Public module `skill_resolver` declaration lacks a doc comment. All public items must have documentation. Add a doc comment describing the skill_resolver module.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:29` — Public module `validation` declaration lacks a doc comment. All public items must have documentation. Add a doc comment describing the validation module.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:32` — Public re-export `SkillContext` lacks a doc comment. All public items must have documentation. Add a doc comment describing the re-exported type or reference it from the original module.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:33` — Public re-export `SkillError` lacks a doc comment. All public items must have documentation. Add a doc comment describing the re-exported type.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:34` — Public re-exports from `operations` lack doc comments. All public items must have documentation. Add a doc comment describing the re-exported types.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:35` — Public re-exports from `parse` lack doc comments. All public items must have documentation. Add a doc comment describing the re-exported items.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:36` — Public re-exports from `schema` lack doc comments. All public items must have documentation. Add a doc comment describing the re-exported functions.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:37` — Public re-exports from `skill` lack doc comments. All public items must have documentation. Add a doc comment describing the re-exported types. (This is the one line this commit edited in `lib.rs` — it removed `SAH_INTERNAL_FRONTMATTER_KEYS` from an already-undocumented `pub use`. The missing-doc condition pre-dates the commit.)
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:38` — Public re-export `SkillLibrary` lacks a doc comment. All public items must have documentation. Add a doc comment describing the re-exported type.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:39` — Public re-export `SkillResolver` lacks a doc comment. All public items must have documentation. Add a doc comment describing the re-exported type.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:40` — Public re-exports from `validation` lack doc comments. All public items must have documentation. Add a doc comment describing the re-exported items.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/lib.rs:43` — Public re-exports from `swissarmyhammer_operations` lack doc comments. All public items must have documentation. Add a doc comment describing the re-exported items from the operations crate.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/skill.rs:18` — Public constructor returns `Result<Self, String>` instead of a typed error. Library crates must use `thiserror` so callers can match on specific error cases. Define a typed error enum and return `Result<Self, SkillNameError>` or similar. Create variants for 'empty' and 'invalid_chars' cases so callers can distinguish error types. (Displaced: this commit deleted 19 lines above, moving `SkillName::new` up from ~line 37 to line 18. The signature is unchanged.)
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/skill_loader.rs:41` — The literal `3` is a magic number representing the byte length of the opening frontmatter delimiter `---` and should be a named constant or computed from the delimiter string itself. Replace `&content[3..]` with `&content[OPENING_DELIMITER.len()..]`, where `OPENING_DELIMITER = "---"` is a module-level constant. Alternatively, compute it inline as `&content["---".len()..]`.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/skill_loader.rs:62` — The literal `4` is a magic number representing the byte length of the closing frontmatter delimiter `\n---` and should be a named constant or computed from the delimiter string itself. Replace `end_pos + 4` with `end_pos + CLOSING_DELIMITER.len()`, where `CLOSING_DELIMITER = "\n---"` is a module-level constant. Alternatively, compute it inline as `end_pos + "\n---".len()`.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/skill_loader.rs:73` — Public function returns `Result<Skill, String>` instead of a typed error. Library crates must use `thiserror` so callers can match on specific error cases. Return a typed error enum: `pub fn load_skill_from_dir(dir: &Path, source: SkillSource) -> Result<Skill, SkillLoadError>` or the consolidated error type used by other functions.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/skill_loader.rs:77` — The filename "SKILL.md" is hardcoded in multiple functions (load_skill_from_dir and load_skill_from_builtin) and appears in parsing conditions and error messages. Changes to the filename would require updates in multiple places across the module. Extract as a module-level constant: const SKILL_MD_FILE: &str = "SKILL.md" and use it in dir.join(SKILL_MD_FILE), the find condition, and error messages to maintain a single source of truth.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/skill_loader.rs:89` — Public function returns `Result<Skill, String>` instead of a typed error. Library crates must use `thiserror` so callers can match on specific error cases. Return a typed error enum: `pub fn load_skill_from_builtin(skill_name: &str, files: &[(&str, &str)]) -> Result<Skill, SkillError>` or the consolidated error type used by other functions.
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/skill_loader.rs:110` — Path traversal via unvalidated key in `load_skill_from_builtin`: The `key` constructed from `name.strip_prefix(&prefix)` lacks validation. If a builtin file name contains path traversal sequences like `../`, the key would include them and later escape the base directory when used with `Path::join` in `write_resources`. Validate the `key` before inserting into the HashMap. Reject keys containing `.`, `..`, or path separators that could escape the intended directory: `if key.contains("..") || key.contains("::") { return Err(...); }`. (`load_skill_from_builtin` is untouched by this commit.)
- [ ] [PRE-EXISTING] `crates/swissarmyhammer-skills/src/skill_loader.rs:319` — YAML frontmatter delimiters "---" and "\n---" are hardcoded multiple times in split_frontmatter. Magic numbers 3 (length of "---") and 4 (length of "\n---") are easy to get wrong if the delimiter ever changes, and duplication creates a maintenance hazard. Extract frontmatter delimiters as named constants: `const FM_DELIM: &str = "---"; const FM_DELIM_CLOSED: &str = "\n---";` and replace magic numbers 3 and 4 with FM_DELIM.len() and FM_DELIM_CLOSED.len() respectively.