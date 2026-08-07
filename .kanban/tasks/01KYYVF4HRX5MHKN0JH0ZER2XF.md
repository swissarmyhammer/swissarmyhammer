---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzebkeattkdx9skfpvtzy2y5
  text: |-
    Research done.

    Dependency edge is already there: `crates/mirdan/Cargo.toml` line 25 lists `swissarmyhammer-common = { workspace = true }`. No Cargo change needed. `swissarmyhammer_common::frontmatter` is a `pub mod` and `split_frontmatter_body` is `pub`.

    Prevailing call pattern (five sites): read the file, call `split_frontmatter_body(&content)?`, feed the frontmatter slice straight to `serde_yaml_ng::from_str`. No trim of the slice. `entity/io.rs`, `entity/store.rs`, `tools/health_registry.rs`, `config/model.rs`, `tools/ralph/state.rs` all do this. list.rs follows the same shape.

    Dropping the `content.trim()` in list.rs is part of the fix, not a side effect: `trim()` lets `"  ---\n..."` open a frontmatter block, which is exactly the loose delimiter rule the card removes.

    Three RED cases confirmed against the old code by hand:
    1. `description: Uses --- as a separator` — old `find("---")` cuts at the run inside the value, so the parsed map holds only `name` and a truncated `description: Uses`. `metadata.version` is lost and `read_frontmatter_version` falls back to "latest".
    2. Opening line `---description: leaked` — old `strip_prefix("---")` accepts it, and the remainder parses as valid YAML, so the reader returns keys from a file that has no frontmatter block.
    3. No closing delimiter line, but a `---` run inside a value — old code finds that run and returns a partial map instead of `None`.

    An opening line of exactly `----` is `None` under both old and new code (the old code's leftover `-` makes the YAML unparseable), so that example is a pin, not a RED case. It is still in the test because the acceptance list names it.

    Discovered out of scope: four further copies of the same substring split live in the same crate — `mirdan/src/applier.rs` (`install/applier.rs`), `mirdan/src/mcp_config.rs`, `mirdan/src/info.rs`, `mirdan/src/git_source.rs`. The card scopes this work to `list.rs`, so they get their own card.
  timestamp: 2026-08-07T14:56:26.970402+00:00
- actor: claude-code
  id: 01kzec1kxdeq83pds0fn9941xb
  text: |-
    Implementation landed. RED first, then GREEN.

    RED run (`cargo nextest run -p mirdan --lib list::`): 16 tests, 13 passed, 3 failed. Each failed for the predicted reason:
    - `three_hyphen_run_in_a_description_keeps_every_frontmatter_key` — description read back as "Uses", not "Uses --- as a separator".
    - `an_opening_line_with_text_after_the_hyphens_is_not_a_delimiter` — name read back as Some("test-skill"), not None.
    - `a_file_with_no_closing_delimiter_line_reads_no_frontmatter` — name read back as Some("test-skill"), not None.

    `an_opening_line_of_more_than_three_hyphens_is_not_a_delimiter` passed on the old code, as the research comment predicted. It is a pin for the `----` example the acceptance list names.

    GREEN run: 16 of 16 pass. Every pre-existing test is unedited.

    `parse_frontmatter` now reads the file and calls `split_frontmatter_body(&content)?`. The `content.trim()` is gone: `trim()` let a leading-whitespace line open a frontmatter block, which the line-anchored rule forbids.

    Two further things the rules required, both inside the blast radius of this change:

    1. `cargo clippy -W missing_docs` reported five undocumented public fields on `InstalledPackage` in the file I edited (`name`, `description`, `package_type`, `version`, `targets`). The missing-docs rule is file-wide, not diff-wide, so I documented all five. `list.rs` now reports zero `missing_docs`.

    2. The module documentation of `swissarmyhammer-common/src/frontmatter.rs` enumerated the readers of `split_frontmatter_body` and counted them ("Five readers ... All five therefore agree"). My change made that count wrong. It was already wrong by one before me — commit a193b9427 added the two `swissarmyhammer-config` model-config readers and never updated the count. I named every reader and removed the hand-maintained number from the function docstring, which now points at the module list instead of repeating it. The "other crates carry copies" sentence now says "among them", because templating and merge are not the only ones — skills, agents, validators, sem, code_context, and markdowndown carry copies too.

    Verification, all from the repo root:
    - `cargo nextest run -p mirdan -p mirdan-app -p swissarmyhammer-common` — 1062 passed, 0 failed, 0 skipped.
    - `cargo clippy -p mirdan -p mirdan-app -p swissarmyhammer-common --all-targets --all-features -- -D warnings` — clean.
    - `cargo check --workspace --all-targets` — zero warnings, zero errors.
    - `cargo doc -p mirdan -p swissarmyhammer-common --no-deps` — the warnings it prints are pre-existing and unrelated (`create_temp_dir` in `fs_utils`, unresolved links in mirdan module docs). No link I wrote is among them.
    - `cargo fmt` run on both crates.

    Blast radius: `read_frontmatter_name` is the only public reader. One caller lives outside the crate, `apps/mirdan-app/src/commands.rs`, which matches a skill directory by frontmatter name. The signature does not change. The behavior is stricter only for malformed frontmatter, and it now reads the name correctly for a SKILL.md whose description holds a three-hyphen run, where the truncated YAML used to fail to parse and lose the name outright. mirdan-app tests pass.

    Filed `^nk1yr64` for the four remaining copies of this defect in the same crate: `install/applier.rs`, `mcp_config.rs`, `info.rs`, `git_source.rs`. `applier.rs` is the worst — it `.expect("SKILL.md must open with frontmatter")`, so a bad read panics.

    Unrelated observation, not touched: a stray `.kanban` directory sits at `crates/swissarmyhammer-mcp-proxy/.kanban`. It dates to Jul 31 and nothing in it is newer than this session, so this work did not create it.
  timestamp: 2026-08-07T15:04:11.437938+00:00
- actor: claude-code
  id: 01kzec1vkw20cgkwz8bxd5w597
  text: |-
    ### implement — changed
    - evidence: 2 files — crates/mirdan/src/list.rs, crates/swissarmyhammer-common/src/frontmatter.rs. 4 new tests (3 RED first, 1 pin). `cargo nextest run -p mirdan -p mirdan-app -p swissarmyhammer-common`: 1062 passed, 0 failed, 0 skipped. `cargo clippy --all-targets --all-features -- -D warnings`: clean. `cargo check --workspace --all-targets`: zero warnings. Follow-up card `^nk1yr64`.
    - next: /review
  timestamp: 2026-08-07T15:04:19.324828+00:00
- actor: claude-code
  id: 01kzecxp82zftm6xh5sbm88t37
  text: |-
    ### review — findings
    - evidence: 6 findings — crates/mirdan/src/list.rs:43 (x2), crates/mirdan/src/list.rs:427, crates/mirdan/src/list.rs:666, crates/mirdan/src/list.rs:684, crates/mirdan/src/list.rs:694
    - scope: review sha HEAD~1..HEAD (commit 60f6a1247)
    - engine counts: 10 findings, 10 confirmed; 1 of 9 review tasks failed, so the results are incomplete
    - dropped: 4 findings that asked to rename pre-existing tests in crates/swissarmyhammer-common/src/frontmatter.rs (skill rule blocks refactoring test code that already existed)
    - next: fix the 6 open findings, then re-run review
  timestamp: 2026-08-07T15:19:31.330868+00:00
- actor: claude-code
  id: 01kzecz0q75zv3v4z8xbs5ejch
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — crates/mirdan/src/list.rs, crates/swissarmyhammer-common/src/frontmatter.rs; 4 new tests, 3 RED first
    - test: green — cargo nextest run --workspace, 13587 passed, 0 failed, 0 skipped; fmt and clippy clean
    - commit: 60f6a1247
    - review: findings — crates/mirdan/src/list.rs:43 (cognitive complexity 72 of 15, nesting 6 of 4), crates/mirdan/src/list.rs:43 (four adjacent bool parameters), crates/mirdan/src/list.rs:427 (sibling copies in git_source.rs, info.rs, mcp_config.rs still substring-parse), crates/mirdan/src/list.rs:666, crates/mirdan/src/list.rs:684, crates/mirdan/src/list.rs:694 (new tests omit the `test_` prefix)
  timestamp: 2026-08-07T15:20:14.823701+00:00
- actor: claude-code
  id: 01kzeda38h82b908d3zxz1t2m1
  text: |-
    Picked the card back up from `review` with six open findings. Research for the fix.

    **Finding 1+2 (complexity, bool parameters).** `discover_packages` and `run_list` both carry the same four adjacent bools. The finding names one example, so both change. The four CLI flags combine (`--skills --tools` scans two types), so an enum with one variant cannot model them. A `PackageFilter` that holds the selected types, empty meaning "every type", does.

    Callers of `list::discover_packages`: `outdated.rs` (two), `apps/mirdan-app/src/commands.rs` (one), `install/tests.rs` (six), plus `run_list`. Callers of `run_list`: `dispatch.rs` (one) and three tests in `list.rs`. `git_source::discover_packages` is a different function with the same name and is untouched.

    `registry_url` and the lockfile enrichment loop both walk the same lockfile dirs and match a key by its last path segment. Extracting the enrichment loop alone would leave two copies of that walk, so both call one shared key lookup. The two matched slightly differently — enrichment on `last_segment == name`, `registry_url` on `last_segment == name || key == name`. Unifying on the wider test is behavior-preserving for enrichment, which only runs when `source == name`, so a whole-key match already sets the field to what it holds.

    Loading each lockfile once per call, not once per package, keeps the current disk cost. Home directory is read before the current directory, and first match wins, which is the order the nested loops produced.

    **Finding 3 (sibling copies).** Five production sites plus one test helper, all `strip_prefix("---")` then `find("---")`:

    | site | old failure |
    |---|---|
    | `install/package.rs::read_frontmatter` | hides the delimiter behind `FRONTMATTER_DELIMITER`; returns `RegistryError::Validation` |
    | `install/applier.rs::frontmatter_map` | test helper, `.expect(...)` on both halves, and its `find("\n---")` is anchored on the left only |
    | `mcp_config.rs::parse_yaml_frontmatter` | `RegistryError::Validation` |
    | `info.rs::read_frontmatter_field` | `"unknown"` fallback |
    | `git_source.rs::extract_name_from_frontmatter` | `None` |

    `install/uninstall.rs::read_skill_frontmatter_name` delegates to `read_frontmatter`, so it inherits the fix.

    Three RED shapes confirmed by hand against the old code:
    1. `description: >-` block scalar holding an indented `---`. `find("---")` cuts at that run, the truncated text still parses, and `metadata.version` is lost.
    2. First line `---description: leaked`. `strip_prefix("---")` accepts it and the remainder parses, so a file with no frontmatter block yields keys.
    3. First line `----`. Both old and new reject it, the old by leaving a stray `-` that makes the YAML unparseable. It is a pin, not a RED case, and the acceptance list names it.

    The fixtures go in one `#[cfg(test)] mod frontmatter_fixtures` in `mirdan`, so the six sites share one statement of the delimiter rule instead of six near-copies of the same markdown.

    **Finding 4 (test names).** Four test functions in `list.rs` omit the `test_` prefix, not three: the review scope missed `a_file_with_no_closing_delimiter_line_reads_no_frontmatter`. All four get the prefix.

    This work also satisfies `^t7gabdj`.
  timestamp: 2026-08-07T15:26:17.873855+00:00
- actor: claude-code
  id: 01kzee9f4ey3g51224e83tpnzv
  text: |-
    All six findings closed. RED first, then GREEN.

    **RED run** (`cargo nextest run -p mirdan --lib` over the new tests): 23 tests, 12 failed. Each failed for the predicted reason.

    - `install::tests::test_read_frontmatter_*` (3) — the three-hyphen run cut the frontmatter, `read_frontmatter` returned `Err("missing 'name'")` where the test wants `("test-skill", "1.2.3")`; `---description: leaked` and the unterminated file both returned `Ok`.
    - `mcp_config::tests::test_parse_yaml_frontmatter_*` (3) — same shapes.
    - `info::tests::test_read_frontmatter_field_*` (3) — same shapes, `"unknown"` contract.
    - `git_source::tests::test_extract_name_*` (3) — same shapes, `None` contract.

    The four `..._rejects_an_opening_line_of_four_hyphens` tests passed on the old code and are pins, not RED cases: the old opener left a stray `-` that made the YAML unparseable, so both openers reject `----`. The acceptance list names the case, so the pins stay.

    The first draft of the shared fixture put `name` before `description`. That made `git_source::test_extract_name_reads_past_a_three_hyphen_run` pass on the old code, because the truncation happened after `name`. Moving `name` and `metadata` after the block scalar made the loss observable through every reader, including the one that exposes only `name`. Do not reorder them back.

    `applier.rs::frontmatter_map` changed return type, so its four tests could not compile against the old signature. Their RED is the missing contract; the behavior they assert is the same as the four production sites, byte for byte, because all six read the same fixtures.

    **GREEN run**: 49 of 49 frontmatter tests pass.

    ## What changed

    Finding 3 — six sites now call `swissarmyhammer_common::frontmatter::split_frontmatter_body`:

    | site | contract kept |
    |---|---|
    | `install/package.rs::read_frontmatter` | `RegistryError::Validation`; the `FRONTMATTER_DELIMITER` constant is gone, because the delimiter rule now lives in the splitter |
    | `install/applier.rs::frontmatter_map` | returns `Result<Mapping, String>` instead of `.expect(...)`; the two call sites name the file in the panic message, which the old `.expect` did not |
    | `mcp_config.rs::parse_yaml_frontmatter` | `RegistryError::Validation` |
    | `info.rs::read_frontmatter_field` | `"unknown"` |
    | `git_source.rs::extract_name_from_frontmatter` | `None` |
    | `install/uninstall.rs::read_skill_frontmatter_name` | unchanged; it delegates to `read_frontmatter` |

    The four markdown fixtures live once, in `crates/mirdan/src/frontmatter_fixtures.rs`, a `#[cfg(test)]` module. Six sites assert the same four files against their own error contracts, so the delimiter rule is stated once instead of six times.

    Finding 2 — `PackageFilter` replaces the four bools in both `discover_packages` and `run_list`. The CLI flags combine, so an enum with one variant cannot model them; the filter holds the selected types and an empty selection covers every type. `PackageFilter::from_flags` takes `(bool, PackageType)` pairs, so the dispatch call site names each flag's type beside it. `PackageType` gained `Hash`, `PartialOrd`, and `Ord` derives so `PackageFilter` can derive its full trait set.

    Finding 1 — `discover_packages` is now four `if` statements at depth 1. The scanning blocks became `scan_skill_stores`, `scan_agent_skill_dirs`, `scan_validator_dirs`, `scan_tool_dirs`, `scan_agent_plugin_dirs`, and `scan_one_agents_plugin_dirs`; the repeated "load the agents config, resolve the targets" preamble became `target_agents`.

    The lockfile walk needed care. Extracting only the enrichment loop would have left `registry_url` holding a second copy of the same walk, so both now call `lockfile_keys` and `find_lockfile_key`. Two details are deliberate:

    - The two functions matched differently. Enrichment tested `last_segment == name`; `registry_url` tested `last_segment == name || key == name`. The shared lookup uses the wider test. That does not change enrichment, which only runs when `source == name`, so a whole-key match writes back the value the field already holds.
    - `lockfile_keys` reads each lockfile once for each call, not once for each package. The old nested loop read them twice in total; a naive per-package lookup would have read them twice per package.

    Finding 4 — four test functions renamed, not three. The review scope missed `a_file_with_no_closing_delimiter_line_reads_no_frontmatter`, which has the same defect.

    ## Blast radius

    `discover_packages` and `run_list` both changed signature. Every caller was updated: `outdated.rs` (two), `dispatch.rs` (one), `apps/mirdan-app/src/commands.rs` (one), `install/tests.rs` (six), `list.rs` tests (three). `git_source::discover_packages` shares the name but is a different function and is untouched. `registry_url` keeps its signature.

    ## Verification

    - `cargo nextest run --workspace` — 13607 passed, 0 failed, 0 skipped.
    - `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
    - `cargo clippy -p mirdan -p mirdan-app -- -W missing_docs` — zero findings in any file this change touched. That required documenting the five `PackageType` variants and the seven public fields of `McpServerEntry` and `McpFrontmatter`, because the missing-docs rule reads the whole file, not the diff.
    - `cargo doc -p mirdan --no-deps` — 20 warnings, all pre-existing unresolved module links; none names a symbol or link this change wrote.
    - `cargo fmt --all`.

    ## Note

    `^nk1yr64`, the follow-up card the last pass filed for these same four copies, was archived during this session by something outside this agent. Its content is covered here and on `^t7gabdj`.
  timestamp: 2026-08-07T15:43:25.838270+00:00
- actor: claude-code
  id: 01kzeeaywbzyy9byaecvsyeh42
  text: |-
    ### implement — changed
    - evidence: 13 files — crates/mirdan/src/{list.rs, mcp_config.rs, info.rs, git_source.rs, package_type.rs, dispatch.rs, outdated.rs, lib.rs, frontmatter_fixtures.rs}, crates/mirdan/src/install/{package.rs, applier.rs, tests.rs}, apps/mirdan-app/src/commands.rs. 20 new tests, 12 RED first, 4 pins, 4 RED by missing contract. All 6 findings checked. `cargo nextest run --workspace`: 13607 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean. `cargo clippy -p mirdan -W missing_docs`: zero findings in changed files. `cargo fmt --all`.
    - next: /review
  timestamp: 2026-08-07T15:44:14.731277+00:00
- actor: claude-code
  id: 01kzefcpqc42stnqgr6maj55fe
  text: |
    ### review — findings

    - scope: `review sha HEAD~1..HEAD` (commit 6f5504fd2), round 2
    - counts: 11 findings, 11 confirmed, 3 refuted, 27 attempted, 0 failed
    - round 1: all six items verified closed. `PackageFilter` at list.rs:42; `discover_packages` decomposed into `scan_skill_stores`, `scan_agent_skill_dirs`, `scan_validator_dirs`, `scan_tool_dirs`, `scan_agent_plugin_dirs`, `lockfile_keys`, `enrich_sources_from_lockfiles`; no `strip_prefix("---")` split remains in mirdan production code; every test function in list.rs carries the `test_` prefix.
    - evidence: crates/mirdan/src/git_source.rs:100, crates/mirdan/src/git_source.rs:355, crates/mirdan/src/info.rs:168, crates/mirdan/src/install/package.rs:310, crates/mirdan/src/list.rs:382, crates/mirdan/src/list.rs:408, crates/mirdan/src/list.rs:434, crates/mirdan/src/list.rs:460, crates/mirdan/src/mcp_config.rs:117, crates/mirdan/src/mcp_config.rs:160, crates/mirdan/src/mcp_config.rs:180
    - next: apply the 11 new findings; task stays in `review`.
  timestamp: 2026-08-07T16:02:40.492882+00:00
- actor: claude-code
  id: 01kzefdrs51y0vc8zrshpmb434
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 13 files; 20 new tests, 12 RED first; all six round-1 findings closed
    - test: green — cargo nextest run --workspace, 13607 passed, 0 failed, 0 skipped; clippy -D warnings clean; cargo fmt --all
    - commit: 6f5504fd2
    - review: findings — 11 new, 11 confirmed, 3 refuted, 27 attempted, 0 failed. crates/mirdan/src/git_source.rs:100, crates/mirdan/src/git_source.rs:355, crates/mirdan/src/info.rs:168, crates/mirdan/src/install/package.rs:310, crates/mirdan/src/list.rs:382, crates/mirdan/src/list.rs:408, crates/mirdan/src/list.rs:434, crates/mirdan/src/list.rs:460, crates/mirdan/src/mcp_config.rs:117, crates/mirdan/src/mcp_config.rs:160, crates/mirdan/src/mcp_config.rs:180
    - round-1 verification: all six prior findings confirmed closed by the reviewer

    Note: the checkpoint commit swept an unrelated edit to `builtin/skills/finish/SKILL.md` into this task's range. That edit is not task scope.
  timestamp: 2026-08-07T16:03:15.365288+00:00
- actor: claude-code
  id: 01kzefv17rg2p20193wdy2wtkf
  text: |-
    Picked the card back up from `review` with eleven open findings from round 2. Research for the fix.

    **Findings 3+4 (git_source.rs:514 / info.rs:168 / install/package.rs:310, the split-parse-extract wrapper).** Round 1 consolidated the *split* call. Five readers still each carry the parse-and-extract wrapper around it:

    | site | contract |
    |---|---|
    | `list.rs::parse_frontmatter` | `Option<Value>` |
    | `mcp_config.rs::parse_yaml_frontmatter` | `Result<Value, RegistryError>` |
    | `install/package.rs::read_frontmatter` | `Result<(name, version)>`, distinct error for a missing `name` |
    | `info.rs::read_frontmatter_field` | `String`, `"unknown"` fallback |
    | `git_source.rs::extract_name_from_frontmatter` | `Option<String>`, takes content not a path |

    A new `crate::frontmatter` module owns read-split-parse once and the two field lookups (top level, and `metadata.<field>` first with a top-level fallback -- the shape `info.rs`, `list.rs`, and `package.rs` all wanted for `version`). Each site keeps a wrapper that states only its own error contract, so every existing test keeps its target. `frontmatter_fixtures.rs` becomes `frontmatter::fixtures`, which puts the delimiter rule and the four files that state it in one module.

    **Findings 5-8 (list.rs:382/:408/:434/:460, the four scan walkers).** The four differ in three things only: the manifest file, the package type, and whether a `rules/` directory must also be there. A `PackageScan` row carries those, and one `scan_package_dirs` walker reads them. The manifest name is not a fourth field: `PackageType` already knows its manifest in `detect_package_type`, so `PackageType::manifest_file()` states it once and `detect_package_type`, the `list.rs` scans, `git_source::scan_dir_for_package`, and the two `match pkg_type` version tables in `install/package.rs` all read it from there.

    Two behaviour differences between the four are deliberate and stay: `scan_skills` names a package after its directory, while `scan_skills_recursive` prefers the frontmatter name; and plugins read their name from `plugin.json`, carry no description, and version `latest`. `scan_skills_recursive` is a recursive store walker, not one of the four, and keeps its own shape.

    None of the four walkers has a direct unit test today, so the generic walker gets one for each of the four types plus the validator `rules/` requirement.

    **Findings 9-11 (mcp_config.rs, the `ServersKey`/`ToolName` newtypes).** Four signatures carry the adjacent pair, not three: `unregister_mcp_server` has it too. `agents::McpConfig::servers_key` stays a `String` -- it is a serde field read from `builtin/agents/*.yaml`, and retyping it reaches `agents.rs`, `status.rs`, `sync.rs`, and `deploy.rs`, none of which a finding names. Call sites wrap at the boundary. The two newtypes are the same code twice, so one `macro_rules!` writes both; the duplication rule names a macro as the single source.

    Call sites to update: `install/profile.rs` (2), `install/deploy.rs` (2), `install/package.rs` (1), `install/uninstall.rs` (1), `strategy/mod.rs` (3), plus eight tests in `mcp_config.rs`.

    **Finding 1 (git_source.rs:100, `parse_git_source`, cognitive complexity 19).** Three shapes in one function. Each becomes a parser returning `Option<GitSource>`, and the function is an `or_else` chain ending in the one error. The repeated `parts[n].chars().all(...)` test becomes one predicate.

    **Finding 2 (git_source.rs:355, `discover_packages`, complexity 23, nesting 5).** The subpath branch returns its own result, so it becomes `discover_in_subpath`. The priority-directory walk is `read_dir` inside `is_dir` inside a loop -- the `is_dir()` guard is redundant, because `read_dir` on a non-directory fails, so dropping it and extracting `scan_child_dirs` flattens three levels to one.
  timestamp: 2026-08-07T16:10:30.008697+00:00
- actor: claude-code
  id: 01kzej5qc1arh9ctd567cv632a
  text: |-
    All eleven round-2 findings closed. RED first for every new contract.

    **RED runs.**
    - `PackageType::manifest_file()` — `cargo nextest run -p mirdan --lib package_type::` failed to compile: `no method named manifest_file found for enum PackageType`. GREEN: 14 of 14.
    - `crate::frontmatter` — the module's fourteen tests failed to compile with `cannot find function parse / parse_file / read_file / field / metadata_field / file_field / file_metadata_field in this scope`. GREEN: 14 of 14.
    - `scan_package_dirs` and the four `PackageScan` rows — the nine new `list.rs` tests failed to compile against the absent symbols. GREEN: all pass.
    - `ServersKey` / `ToolName` — the two new `mcp_config.rs` tests failed to compile: `cannot find ServersKey in this scope`.

    Twenty-three tests added: 14 in `frontmatter`, 9 in `list.rs` for the generic walker, 3 in `git_source.rs` for `scan_dir_for_package`, 2 in `mcp_config.rs` for the newtypes, 1 in `package_type.rs` binding `manifest_file()` to detection. Not one pre-existing test's assertion changed; the tests that changed are only the ones whose function signature changed.

    **Two discoveries worth keeping.**

    1. Consolidating the readers made two symbols dead, and clippy caught it under `-D warnings` before I did: `git_source::extract_name_from_frontmatter` and `frontmatter::str_field`. Once `scan_dir_for_package` read by path through `frontmatter::file_field`, the content-taking pair had no production caller left and survived only through its own tests. Deleted both. The lesson for the next agent: after routing a reader through a shared helper, check whether the old wrapper still has a non-test caller -- a wrapper kept alive only by its tests is dead code that reads as covered.

    2. The three `git_source` tests that fed content strings to the deleted wrapper became `scan_dir_for_package` tests instead of being dropped. They now assert the same three shapes -- a named skill, frontmatter with no name, no frontmatter at all -- through the production discovery path rather than through a wrapper that no longer exists.

    **What did not work.** The first draft kept `extract_name_from_frontmatter` as a one-line alias for `str_field` and had `scan_dir_for_package` read the file itself. That keeps every test valid but leaves a wrapper whose only content is a field name, and it re-adds the read the shared module exists to own. Delegating by path is shorter and leaves nothing to drift.

    **Blast radius.** Four `mcp_config` signatures changed; nine call sites across `install/profile.rs`, `install/deploy.rs`, `install/package.rs`, `install/uninstall.rs`, and `strategy/mod.rs` were updated. `cargo check --workspace --all-targets` confirms no other crate calls them. `list::read_frontmatter_name` keeps its signature; its one out-of-crate caller, `apps/mirdan-app/src/commands.rs`, is unaffected. `PackageType` gained a method, which breaks nothing.

    **Verification, all from the repo root.**
    - `cargo nextest run --workspace` — 13628 passed, 0 failed, 0 skipped.
    - `cargo clippy --workspace --all-targets --all-features -- -D warnings` — zero errors.
    - `cargo clippy -p mirdan -p mirdan-app -- -W missing_docs` — zero findings in any file this change touched. The remaining ones in the crate are in `registry/types.rs` and other untouched files.
    - `cargo doc -p mirdan --no-deps` — one `links to private item` warning remains, at `git_source.rs:217` for `[checkout_ref]`. It predates this change; the two I introduced by linking `[frontmatter]` from public docs are fixed.
    - `cargo fmt --all`.

    **Note on process.** Two sub agents did the mechanical parts in parallel: the `ServersKey`/`ToolName` migration with its nine call sites, and the `install/package.rs` rewiring. Both edited `install/package.rs` -- one the MCP call site, the other `read_frontmatter` and the version tables. Both changes are present and the file compiles, but a concurrent whole-file write is a real clobber risk; give overlapping files to one agent next time.
  timestamp: 2026-08-07T16:51:17.505343+00:00
- actor: claude-code
  id: 01kzej5zrartv2ezwtrwnxhv5y
  text: |-
    ### implement — changed
    - evidence: 15 files — crates/mirdan/src/{frontmatter.rs (new), frontmatter/fixtures.rs (moved from frontmatter_fixtures.rs), list.rs, git_source.rs, info.rs, mcp_config.rs, package_type.rs, lib.rs}, crates/mirdan/src/install/{package.rs, applier.rs, profile.rs, deploy.rs, uninstall.rs}, crates/mirdan/src/strategy/mod.rs. 23 new tests, every new contract RED first. All 11 findings checked. `cargo nextest run --workspace`: 13628 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets --all-features -- -D warnings`: zero errors. `cargo clippy -p mirdan -W missing_docs`: zero findings in changed files. `cargo fmt --all`.
    - next: /review
  timestamp: 2026-08-07T16:51:26.090752+00:00
- actor: claude-code
  id: 01kzekpe9p5h471rqcvd0nehb5
  text: |
    ### review — findings

    - scope: `review sha HEAD~1..HEAD` (commit a95954c6c), round 3
    - counts: 27 attempted, 5 confirmed, 3 refuted, 0 failed
    - evidence: crates/mirdan/src/git_source.rs:16, crates/mirdan/src/git_source.rs:416, crates/mirdan/src/install/package.rs:299, crates/mirdan/src/list.rs:540, crates/mirdan/src/new.rs:136
    - dropped: crates/mirdan/src/git_source.rs:1143 asked to name the magic number in a pre-existing test assertion (blame 613f58ef3a, 2026-02-23). The rule against refactoring test code that already existed drops it.
    - prior rounds: all 17 items verified closed against the code at a95954c6c, not only against the resolution notes. One round-2 resolution claim was inaccurate — `VALIDATOR_RULES_DIR` does not name `rules/` once, because `new.rs` still hardcodes the literal. Carried forward as an open item.
    - next: close the five open items, then re-review.
  timestamp: 2026-08-07T17:17:53.846420+00:00
- actor: claude-code
  id: 01kzekr18skp86361krjre1yde
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 15 files; new `crates/mirdan/src/frontmatter.rs` owns read-split-parse once; 23 new tests; all eleven round-2 findings closed
    - test: green — cargo nextest run --workspace, 13628 passed, 0 failed, 0 skipped; clippy -D warnings clean; cargo fmt --all
    - commit: a95954c6c
    - review: findings — 5 new, 27 attempted, 5 confirmed, 3 refuted, 0 failed. crates/mirdan/src/git_source.rs:16, crates/mirdan/src/git_source.rs:416 (path traversal on a user-supplied subpath), crates/mirdan/src/install/package.rs:299, crates/mirdan/src/list.rs:540, crates/mirdan/src/new.rs:136
    - prior-finding verification: all 17 confirmed closed against the code at a95954c6c, not from the resolution notes

    Finding count per round: 6, then 11, then 5. Each round the reviewer reads the surface the previous round's refactor created. No finding has repeated, so the three-round guardrail has not fired.

    The `ralph` keep-alive reached its 50-iteration cap at this point.
  timestamp: 2026-08-07T17:18:46.041870+00:00
- actor: claude-code
  id: 01kzenyrr908z41hsqxj5afqgq
  text: |-
    Picked the card back up from `review` with five open findings from round 3. Research for the fix.

    **Finding 1 (git_source.rs:16, `InstallSource` has no `Clone`).** The cause is a public type that omits a trait every field supports, which orphan rules keep a downstream crate from adding. Three public types live in this file. `InstallSource` had `Debug, PartialEq`; `GitSource` had `Debug, Clone, PartialEq`; `DiscoveredPackage` had `Debug, Clone`. Every field of all three is `String`, `Option<String>`, `PathBuf`, or `PackageType`, and all of those carry `Clone`, `Eq`, and `Hash`. All three now derive `Debug, Clone, PartialEq, Eq, Hash`, so the finding is applied to the cause, not only to the named line. `Default` and `Ord` are not applicable: no variant is a sensible default and no field pair states an order.

    **Finding 2 (git_source.rs:416, path traversal).** `discover_in_subpath` joined `repo_dir.join(subpath)` with no check. The subpath is public API -- `GitSource::subpath` -- and `install/package.rs` passes it straight from the install spec. The directory it indexes is a freshly cloned third-party repository, so two different escapes reach the same scan:

    1. The text names a location outside the clone: `../../etc`, or an absolute path (`Path::join` with an absolute argument discards the root entirely).
    2. The text is ordinary but the repository carries a symbolic link that resolves outside the clone. `git clone` writes symbolic links from repository content, so this is attacker-supplied too, and it reaches the priority-directory walk and the recursive walk as well as the subpath.

    One check cannot cover both, so the fix states each once. `subpath_stays_inside` reads the text and accepts only `Normal` and `CurDir` components. `RepoScan` owns the canonical repository root and answers `contains` for a directory by canonicalizing it and testing the prefix, which follows every link.

    `RepoScan` also removes the reason the check was hard to place. The four walkers each threaded `packages: &mut Vec<_>` and `seen: &mut HashSet<_>`; adding a `root: &Path` beside `dir: &Path` would have put two adjacent `&Path` parameters of different meaning in four signatures, which is the newtype defect round 2 closed for `&str`. The struct carries root, packages, and seen together, so the containment check has one home and the parameter pair is gone.

    Note for the next agent: canonicalize BOTH sides. On macOS a `tempfile::tempdir()` path is `/var/folders/...` while its canonical form is `/private/var/folders/...`, so a prefix test against a raw root matches nothing.

    **Finding 3 (install/package.rs:299, unnamed `(String, String)`).** Two functions in that file return an unnamed tuple of name and version: `read_frontmatter` and `parse_package_spec`. Both get a struct, `FrontmatterMetadata` and `PackageSpec`. `parse_package_spec` is `pub`; a repo-wide grep shows its only callers are `install/package.rs` and `install/tests.rs`, so no other crate sees the change. `read_frontmatter` is `pub(crate)` with callers in `install/package.rs` (2), `install/uninstall.rs` (1), and `install/tests.rs` (9).

    **Finding 4 (list.rs:540, `merge_packages` at nesting depth 4).** The inner target loop becomes `add_unique_targets`.

    **Finding 5 (new.rs:136, the `rules/` literal).** The cause is a production literal that duplicates a constant which already names it. `new.rs` carries five: `"rules"` (which `package_type::VALIDATOR_RULES_DIR` names) and the four manifest names `SKILL.md`, `VALIDATOR.md`, `TOOL.md`, and `.claude-plugin/plugin.json` (which `PackageType::manifest_file()` names). The round-2 resolution listed the readers of `manifest_file()` and missed `new.rs` entirely, which is why the round-2 claim did not hold. All five path joins now read the constant. The `println!` tree and the "Next steps" lines keep their literals: they are user-facing output, not paths, and the public-output-contract rule forbids rewording them.
  timestamp: 2026-08-07T17:57:23.849355+00:00
- actor: claude-code
  id: 01kzepqzhhr79hwzaq6b26x330
  text: |-
    All five round-3 findings closed. RED first for the two that state behaviour.

    **RED runs.**
    - `test_install_source_clones` — failed to compile: `no method named clone found for enum git_source::InstallSource`. GREEN after the derive.
    - The four containment tests — each failed by reading a package from outside the repository. Verbatim from the run: `a subpath that climbs out of the repository must be refused, got Ok([DiscoveredPackage { name: "outside-skill", ... }])`, and the same shape for the absolute subpath, the symlinked subpath, and the symlinked priority directory. GREEN after `subpath_stays_inside` and `RepoScan::contains`.
    - `FrontmatterMetadata` and `PackageSpec` — twelve `expected (String, String), found FrontmatterMetadata` errors named every call site before it was updated.

    The five new tests are 60 of 60 in `git_source`, including the twelve network tests that clone real public repositories. Those matter here: they prove the containment check does not refuse a real clone. `anthropics/skills`, `anthropics/claude-plugins-official`, `basecamp/skills`, and `obra/superpowers` all still discover the same packages.

    **Two things worth keeping for the next agent.**

    1. Canonicalize BOTH sides of a containment test. On macOS a `tempfile::tempdir()` path is `/var/folders/...` while its canonical form is `/private/var/folders/...`. A prefix test of a canonical child against a raw root matches nothing, so the check would refuse every directory and every discovery test would fail. The bug is silent in the other direction too: a raw child against a raw root passes a symlinked escape.

    2. The containment check had no home until the walkers became a struct. Threading `root: &Path` into the four free functions would have placed it beside `dir: &Path` -- two adjacent parameters of the same type and different meaning, in four signatures. That is the newtype defect round 2 closed for `&str`, so closing one finding would have opened another. `RepoScan` carries root, packages, and seen together, which removes the `&mut Vec` + `&mut HashSet` pair from four signatures at the same time.

    **What did not work.** The first draft of `discover_in_subpath` relied only on the resolved check in `RepoScan::contains`, with no text check. It refuses a `../` subpath correctly, but the caller cannot then tell "the subpath leaves the repository" from "the subpath holds no package", so both report the same message. The two checks are kept separate on purpose: the text check names the refusal, and the resolved check catches a symbolic link that no text check can see.

    **Blast radius.** `read_frontmatter` and `parse_package_spec` both changed return type. `parse_package_spec` is `pub`; a repo-wide grep shows no caller outside `crates/mirdan`. `discover_packages` keeps its signature but now errors when `repo_dir` does not resolve; its one production caller, `install/package.rs::run_install_git`, passes a `TempDir` path that always resolves. `InstallSource`, `GitSource`, and `DiscoveredPackage` only gained derives, which breaks nothing.

    **Verification, all from the repo root.**
    - `cargo nextest run --workspace` — 13633 passed, 0 failed, 0 skipped.
    - `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
    - `cargo clippy -p mirdan --all-targets -- -W missing_docs` — zero findings in any file this change touched; the rest of the crate's findings are in `registry/types.rs`, untouched.
    - `cargo doc -p mirdan --no-deps` — zero warnings in any touched file. The pre-existing `git_clone` links to private item `checkout_ref` warning was in a file this change edits, so it is fixed rather than left standing.
    - `cargo fmt --all`.

    **Process note.** Two sub agents made the mechanical call-site edits in parallel, on disjoint file sets this time (one took `install/*`, the other took `list.rs` and `new.rs`), which answers the round-2 note about two agents writing the same file. Every edit was verified here by test, clippy, and diff.

    **Unrelated observation, not touched.** The stray `crates/swissarmyhammer-mcp-proxy/.kanban` directory is still present. It predates this session and this work did not create it.
  timestamp: 2026-08-07T18:11:10.001379+00:00
- actor: claude-code
  id: 01kzepr9s8de6590m0acqbjdb5
  text: |-
    ### implement — changed
    - evidence: 6 files — crates/mirdan/src/git_source.rs, crates/mirdan/src/list.rs, crates/mirdan/src/new.rs, crates/mirdan/src/install/package.rs, crates/mirdan/src/install/uninstall.rs, crates/mirdan/src/install/tests.rs. 5 new tests (4 containment, RED first; 1 Clone, RED by compile error). All 5 round-3 findings checked. `cargo nextest run --workspace`: 13633 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean. `cargo clippy -p mirdan -W missing_docs`: zero findings in changed files. `cargo doc -p mirdan --no-deps`: zero warnings in changed files. `cargo fmt --all`.
    - next: /review
  timestamp: 2026-08-07T18:11:20.488231+00:00
position_column: doing
position_ordinal: '8380'
title: mirdan list.rs carries the frontmatter substring-split defect
---
`crates/mirdan/src/list.rs` has a private `parse_frontmatter` with the same defect ^fpcbeth and ^a2ef9wh fixed elsewhere.

```rust
fn parse_frontmatter(path: &Path) -> Option<serde_yaml_ng::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    let content = content.trim();
    let rest = content.strip_prefix("---")?;
    let end = rest.find("---")?;
    let frontmatter = &rest[..end];
    serde_yaml_ng::from_str(frontmatter).ok()
}
```

Two faults, both already fixed in the other copies:

1. `rest.find("---")` finds the three-hyphen **substring** anywhere, not a delimiter line. A `---` inside a description, or a horizontal rule indented in a block scalar, cuts the frontmatter short. The truncated text usually still parses as valid YAML, so the read silently returns a partial mapping instead of failing.
2. `strip_prefix("---")` accepts a first line of `----` or `---x` as an opening delimiter.

## Who reads it

`read_frontmatter_name`, `read_frontmatter_description`, and `read_frontmatter_version` in the same file. They read `SKILL.md`, `VALIDATOR.md`, and `TOOL.md` to build the `mirdan list` output. A skill whose description holds a three-hyphen run can lose its `name` or its `metadata.version` and list wrong.

## Required change

Call `swissarmyhammer_common::frontmatter::split_frontmatter_body`, which is line-anchored, tolerates CRLF, and returns `None` for a malformed block. Do not write a sixth copy.

Check the dependency edge first: confirm `mirdan/Cargo.toml` already lists `swissarmyhammer-common`, and add it if not. ^a2ef9wh chose `swissarmyhammer-common` as the Tier 1 home for this splitter for exactly this reason.

## Acceptance

- `list.rs` builds no frontmatter split of its own.
- RED first: a `SKILL.md` whose `description` holds a three-hyphen run keeps every frontmatter key, `name` and `metadata.version` included.
- A first line of `----` or `---x` is not read as an opening delimiter.
- A file with no closing delimiter line still returns `None`.
- No behavior change for well-formed input: existing tests pass unedited.

Found by the file-wide over-claim scan on ^a2ef9wh. Related: ^tv3692e (the `parse_frontmatter` copy in `swissarmyhammer-common`). #bug

## Review Findings (2026-08-07 10:12)

> Scope: `HEAD~1..HEAD` (commit 60f6a1247).
> 1/9 review tasks failed — results are INCOMPLETE.

- [x] `crates/mirdan/src/list.rs:43` — Function discover_packages exceeds both cognitive complexity and nesting depth gates. Cognitive complexity of 72 is nearly 5× the gate of 15, driven by 18 branches spread across five distinct scanning blocks (skills, validators, tools, plugins). Additionally, max nesting depth of 6 (gate 4) results from the lockfile enrichment loop block (lines 148–166), where three loop levels nest with two conditional checks: for lockfile_dirs → if Lockfile::load → for pkg in merged → if pkg.source → for key in lf.packages → if last_segment. This depth and complexity harm readability and maintainability. Refactor by extracting the lockfile enrichment logic (lines 148–166) into a separate helper function to flatten nesting. Consider further decomposing the main scanning logic by type (e.g. scan_and_enrich_packages) to reduce cognitive load, or use a data structure (tuple of type flag + callback) to eliminate the parallel if blocks.
- [x] `crates/mirdan/src/list.rs:43` — Function has adjacent bool parameters (skills_only, validators_only, tools_only, plugins_only) that are unreadable at call sites. Callers like `discover_packages(false, false, false, false, None)` force readers to check the signature to understand what each flag means. Replace the four bool parameters with a struct or enum representing the filter options. For example: `struct PackageFilter { skills_only: bool, validators_only: bool, tools_only: bool, plugins_only: bool }` and call as `discover_packages(filter, agent_filter)`. Alternatively, use an enum if only one kind is scanned at a time.
- [x] `crates/mirdan/src/list.rs:427` — The frontmatter parsing in parse_frontmatter was fixed to use split_frontmatter_body for line-anchored delimiter matching, but sibling implementations across mirdan were left unchanged. Three near-copies in git_source.rs (extract_name_from_frontmatter), info.rs (read_frontmatter_field), and mcp_config.rs (parse_yaml_frontmatter) still use the old substring-based parsing, creating inconsistent handling of the same frontmatter parsing pattern across the module. The task description acknowledges four total copies need fixing including applier.rs. Apply the same split_frontmatter_body replacement to the other mirdan frontmatter parsers in this change to avoid inconsistent handling, or if intentionally deferred: verify the separate task is created, prioritized, and linked in tracking.
- [x] `crates/mirdan/src/list.rs:666` — Test function `three_hyphen_run_in_a_description_keeps_every_frontmatter_key` breaks the established naming convention. All other test functions in this file use the `test_` prefix (e.g., `test_read_frontmatter_version_skill` on line 487, `test_merge_packages` on line 515, etc.). Rename to `test_three_hyphen_run_in_a_description_keeps_every_frontmatter_key`.
- [x] `crates/mirdan/src/list.rs:684` — Test function `an_opening_line_of_more_than_three_hyphens_is_not_a_delimiter` breaks the established naming convention. All other test functions in this file use the `test_` prefix. Rename to `test_an_opening_line_of_more_than_three_hyphens_is_not_a_delimiter`.
- [x] `crates/mirdan/src/list.rs:694` — Test function `an_opening_line_with_text_after_the_hyphens_is_not_a_delimiter` breaks the established naming convention. All other test functions in this file use the `test_` prefix. Rename to `test_an_opening_line_with_text_after_the_hyphens_is_not_a_delimiter`.

Four further findings asked to rename pre-existing test functions in `crates/swissarmyhammer-common/src/frontmatter.rs` (lines 461, 478, 541, 581). Those tests were not added or changed by this commit, so the review skill rule that blocks refactoring of test code that already existed drops them.

## Resolution (2026-08-07)

All six findings are closed. Each finding was applied to the whole cause, not only the named line:

- The complexity finding named `discover_packages`. Six helpers now carry the work: `scan_skill_stores`, `scan_agent_skill_dirs`, `scan_validator_dirs`, `scan_tool_dirs`, `scan_agent_plugin_dirs`, `scan_one_agents_plugin_dirs`, plus `target_agents`. The lockfile walk is `lockfile_keys` and `find_lockfile_key`, which `enrich_sources_from_lockfiles` and `registry_url` both call, so the walk exists once.
- The bool-parameter finding named `discover_packages`. `run_list` in the same file carried the same four bools and changed with it. Both take `&PackageFilter`.
- The sibling-copy finding named three files. Five production sites and one test helper changed: `install/package.rs`, `install/applier.rs`, `mcp_config.rs`, `info.rs`, `git_source.rs`. `install/uninstall.rs` delegates to `read_frontmatter` and inherits the fix.
- The naming finding named three test functions. A fourth, `a_file_with_no_closing_delimiter_line_reads_no_frontmatter`, had the same defect and was renamed too.

This work also satisfies ^t7gabdj.

## Review Findings (2026-08-07 10:45)

> Scope: `HEAD~1..HEAD` (commit 6f5504fd2). Round 2. Every round-1 item above is verified closed.

- [x] `crates/mirdan/src/git_source.rs:100` — Function `parse_git_source` exceeds cognitive complexity gate. Cognitive complexity 19 exceeds threshold of 15, making the function difficult to understand and maintain. Refactor to reduce complexity: extract common parsing logic for SSH URLs, HTTPS URLs, and GitHub shorthand into separate helper functions; consider a state machine or parsing strategy pattern.
- [x] `crates/mirdan/src/git_source.rs:355` — Function `discover_packages` exceeds both cognitive complexity and condition-nesting depth gates. Cognitive complexity 23 exceeds 15, and condition-nesting depth 5 exceeds 4, creating deeply nested control flow that is hard to reason about. Refactor to reduce nesting: extract the priority-directory scan into a separate function; flatten conditional logic by reversing guards early; consider extracting the deduplication logic into a helper to reduce decision points.
- [x] `crates/mirdan/src/info.rs:168` — Function duplicates frontmatter parsing logic repeated in `git_source.rs::extract_name_from_frontmatter` (line 514) and `install/package.rs::read_frontmatter` (line 310). The pattern—split frontmatter, parse YAML, extract field, convert to string—is identical across all three. Duplication creates maintenance burden when the frontmatter split rule changes (as this change did). Consolidate frontmatter parsing into a shared helper function. The core logic of splitting and YAML parsing is identical; only the field names, error handling strategy (Option vs String vs Result), and return type vary. Extract this to a single function and parameterize the differences.
- [x] `crates/mirdan/src/install/package.rs:310` — Function duplicates frontmatter parsing logic repeated in `git_source.rs::extract_name_from_frontmatter` (line 514) and `info.rs::read_frontmatter_field` (line 168). All three implement the same sequence: split frontmatter with `split_frontmatter_body`, parse YAML with `serde_yaml_ng::from_str`, extract field(s), convert to string. The duplication creates sync risk when the split rule or YAML parsing changes. Extract frontmatter parsing to a shared helper that handles the split-parse-extract sequence. Parameterize for field names and error handling (return Option, String with default, or Result). Have `read_frontmatter` delegate to this helper, eliminating the verbatim duplication.
- [x] `crates/mirdan/src/list.rs:382` — scan_skills (line 382), scan_validators (line 408), and scan_tools (line 434) are near-identical blocks differing only in manifest filename, package type, and one optional directory check. These should be extracted into a single generic scanning function parameterized by manifest name, package type, and optional validation predicates. Extract a generic function `scan_package_type_dir(dir, manifest_file, package_type, extra_check_fn, location, packages)` and call it from scan_skills, scan_validators, and scan_tools, eliminating ~70 lines of duplicated logic.
- [x] `crates/mirdan/src/list.rs:408` — scan_validators is a near-duplicate of scan_skills (line 382) and scan_tools (line 434). The structure, error handling, and metadata extraction are identical; only the manifest filename, optional rules check, and package type differ. Consolidate into the generic function referenced in the line 382 finding.
- [x] `crates/mirdan/src/list.rs:434` — scan_tools is a near-duplicate of scan_skills (line 382) and scan_validators (line 408). Identical structure and logic; differs only in manifest filename and package type. Extract into the shared generic function.
- [x] `crates/mirdan/src/list.rs:460` — scan_plugins shares the same directory-scanning and package-creation boilerplate with scan_skills (line 382), scan_validators (line 408), and scan_tools (line 434). All four follow: read_dir with match/error handling, flatten entries, is_dir check, manifest file existence check, metadata extraction, then InstalledPackage construction and push. Refactor the directory scanning and package creation boilerplate into a generic helper function that accepts: a validation predicate (for manifest check), a metadata extractor function/closure, and the package type, eliminating the repeated read_dir→flatten→is_dir→manifest→push pattern across all four functions.
- [x] `crates/mirdan/src/mcp_config.rs:117` — Adjacent `&str` parameters with different semantic meanings should use newtypes. `servers_key` (a JSON config object key like "mcpServers") and `tool_name` (a tool identifier like "my-tool") have distinct semantic roles and should not be conflatable at the type level. Create newtype wrappers: `pub struct ServersKey(String)` and `pub struct ToolName(String)`, then update the function signature to `set_mcp_server_entry(..., servers_key: ServersKey, tool_name: ToolName, ...)`. This prevents accidental parameter swapping.
- [x] `crates/mirdan/src/mcp_config.rs:160` — Adjacent `&str` parameters with different semantic meanings should use newtypes. In `remove_mcp_server_entry`, `servers_key` (config key) and `tool_name` (tool identifier) have distinct roles and should not be conflatable. Use ServersKey and ToolName newtypes (create once, reuse across all functions that need them).
- [x] `crates/mirdan/src/mcp_config.rs:180` — Adjacent `&str` parameters with different semantic meanings should use newtypes. `servers_key` and `tool_name` in `register_mcp_server` have the same semantic distinction issue. Use ServersKey and ToolName newtypes in the function signature.

## Resolution round 2 (2026-08-07)

All eleven findings are closed. Each was applied to the whole cause, not only the named line.

**The two complexity findings.** `parse_git_source` is now an `or_else` chain over `parse_ssh_source`, `parse_url_source`, and `parse_shorthand_source`, each returning `Option<GitSource>`, ending in the one error. `split_once_owned` and `is_shorthand_segment` carry the repeated pieces. `git_source::discover_packages` lost its subpath branch to `discover_in_subpath` and its priority-directory walk to `scan_child_dirs`; the `dir.is_dir()` guard before `read_dir` was redundant, so dropping it flattened three nesting levels to one.

**The two duplication findings on the split-parse-extract sequence.** A new module, `crates/mirdan/src/frontmatter.rs`, owns read-split-parse once: `parse`, `parse_file`, `read_file`, `field`, `metadata_field`, `file_field`, `file_metadata_field`. Every reader now delegates and holds no parsing of its own -- `list.rs`, `info.rs`, `mcp_config.rs`, `install/package.rs`, and `git_source.rs`. Each keeps only the wrapper that states its own error contract (`Option`, `"unknown"`, `RegistryError`). `frontmatter_fixtures.rs` moved to `frontmatter::fixtures`, so the delimiter rule and the four files that state it live in one module.

Two things fell out of the consolidation and were deleted, not left dead: `git_source::extract_name_from_frontmatter` (scanning now calls `frontmatter::file_field` by path) and `frontmatter::str_field`, its only caller. Their delimiter tests are covered once in `frontmatter`'s own tests; the three content-shape tests they carried became `scan_dir_for_package` tests, which read through the production path instead of the deleted wrapper.

**The four scan-walker findings.** One `scan_package_dirs` walker reads a `PackageScan` row. Four rows -- `SKILL_SCAN`, `VALIDATOR_SCAN`, `TOOL_SCAN`, `PLUGIN_SCAN` -- carry the package type, the directories a package must hold beside its manifest, and the metadata reader. The manifest name is not a fifth field: `PackageType::manifest_file()` now states each manifest once, and `detect_package_type`, the `list.rs` scans, `git_source::scan_dir_for_package`, `info::show_package_at`, and the two version tables in `install/package.rs` all read it from there. `package_type::VALIDATOR_RULES_DIR` names the `rules/` directory once.

**The three newtype findings.** `ServersKey` and `ToolName` are declared by one `string_newtype!` macro, so the two are one source rather than two copies. Four signatures take them, not three: `unregister_mcp_server` carried the same adjacent pair. Nine call sites wrap at the boundary across `install/profile.rs`, `install/deploy.rs`, `install/package.rs`, `install/uninstall.rs`, and `strategy/mod.rs`. `agents::McpConfig::servers_key` stays a `String`: it is a serde field read from `builtin/agents/*.yaml`.

**Causes removed beyond the named lines.** `info::show_local_info` held the same read-version, read-description, print-three-lines block three times; it is now `show_package_at`. `info` named `"unknown"` twice and now has one `UNKNOWN`. `install/package.rs` named `"0.0.0"` four times and now has one `DEFAULT_VERSION`. `list.rs` named `"latest"` twice and now has one `UNKNOWN_VERSION`.

**Not changed, and why.** `install/applier.rs::frontmatter_map` still calls `split_frontmatter_body` itself. It is a `#[cfg(test)]` helper that parses into `serde_yaml_ng::Mapping` rather than `Value`, because the duplicate-key rejection it asserts is a property of `Mapping`. Routing it through the shared reader would drop that check. No finding names it.

## Review Findings (2026-08-07 11:53)

> Scope: `HEAD~1..HEAD` (commit a95954c6c). Round 3. Every round-1 and round-2 item above is verified closed against the code at this commit, not only against the resolution notes.

- [x] `crates/mirdan/src/git_source.rs:16` — Public enum `InstallSource` is missing the `Clone` trait, which it must implement because all its fields are cloneable (String and GitSource, which derives Clone). Downstream crates cannot add Clone due to orphan rules — it must be in the original definition. Change line 16 from `#[derive(Debug, PartialEq)]` to `#[derive(Debug, PartialEq, Clone)]`.
- [x] `crates/mirdan/src/git_source.rs:416` — Path traversal vulnerability: user-controlled `subpath` parameter is joined with `repo_dir` without validation. An attacker could provide paths like `../../../etc/passwd` to read files outside the repository. Validate that the computed path stays within `repo_dir` after joining. Example: use `std::fs::canonicalize()` on both paths and verify one is a prefix of the other, or use a path component validator that rejects `..` and absolute paths in `subpath`.
- [x] `crates/mirdan/src/install/package.rs:299` — read_frontmatter returns an unnamed tuple (String, String) where both elements have different semantic meanings (package name vs version). Unnamed tuples with semantically distinct elements are error-prone and fail to document intent. Create a struct to represent the result: pub(crate) struct FrontmatterMetadata { pub name: String, pub version: String } and return Result<FrontmatterMetadata, RegistryError>. This prevents accidental reordering, documents the tuple elements by name, and follows the newtype pattern for semantic clarity.
- [x] `crates/mirdan/src/list.rs:540` — Function has max condition-nesting depth of 4, meeting the gate threshold of 4. Excessive nesting reduces readability and increases cognitive load for maintainers. Extract the inner loop logic (lines 545-549) into a separate helper function to reduce nesting depth. For example, create `fn add_unique_targets(existing: &mut InstalledPackage, targets: &[String])` to eliminate one level of nesting.

A fifth finding asked to name a magic number, `plugins.len() >= 10`, in a test assertion at `crates/mirdan/src/git_source.rs:1143`. `git blame` dates that line to commit 613f58ef3a (2026-02-23), so it is test code that already existed and this commit did not touch it. The review skill rule that blocks refactoring of pre-existing test code drops it.

`list.rs:540` is `merge_packages`, production code. `git blame` dates it to commit 7209f61c55, so it is pre-existing production code that this commit shifted but did not change. The pre-existing-test rule does not reach production code, so the finding stands.

### Closure check on the round-2 resolution

All seventeen prior items are closed. Verified in code: `discover_packages` in `list.rs` is about twenty lines at nesting depth 2 with all seven named helpers present; `PackageFilter` carries the four former bools through both `discover_packages` and `run_list`; no `strip_prefix("---")` plus `find("---")` pair survives anywhere under `crates/mirdan/src`; every `#[test]` function in `list.rs` carries the `test_` prefix; `parse_git_source` is a six-line `or_else` chain; `git_source::discover_packages` is flat at depth 2; `crates/mirdan/src/frontmatter.rs` declares all seven readers and every call site delegates; `extract_name_from_frontmatter` and `str_field` are gone with no callers; the four `scan_*` functions are gone in favour of `scan_package_dirs` over four `PackageScan` rows; and all four `mcp_config.rs` signatures take `ServersKey` and `ToolName`.

One claim in the round-2 resolution does not hold as written, so it is carried forward as an open item:

- [x] `crates/mirdan/src/new.rs:136` — The round-2 resolution states that `package_type::VALIDATOR_RULES_DIR` names the `rules/` directory once. It does not. `new.rs` still hardcodes `let rules_dir = base_dir.join("rules");`, the only remaining production occurrence of the bare `"rules"` literal. Replace it with `package_type::VALIDATOR_RULES_DIR` so the constant is the single source it is claimed to be.

## Resolution round 3 (2026-08-07)

All five findings are closed. Each was applied to the whole cause, not only the named line.

**The `Clone` finding.** The cause is a public type that omits a trait every field supports, which orphan rules keep a downstream crate from adding. `git_source.rs` declares three public types, and all three were short. Every field of all three is a `String`, an `Option<String>`, a `PathBuf`, or a `PackageType`, and each of those carries `Clone`, `Eq`, and `Hash`. `InstallSource`, `GitSource`, and `DiscoveredPackage` now all derive `Debug, Clone, PartialEq, Eq, Hash`. `Default` and `Ord` stay off: no variant is a sensible default, and no field pair states an order.

**The path-traversal finding.** The subpath reaches `discover_in_subpath` from the install spec through the public `GitSource::subpath` field, and the directory it indexes is a freshly cloned third-party repository. Two different escapes therefore reach the same walk, and one check cannot cover both:

1. The text names a location outside the clone. `../outside/pkg` climbs out, and an absolute subpath discards the root entirely, because `Path::join` with an absolute argument returns the argument.
2. The text is ordinary but the repository carries a symbolic link that resolves outside the clone. `git clone` writes symbolic links from repository content, so the link is attacker-supplied too, and it reaches the priority-directory walk and the recursive walk as well as the subpath.

`subpath_stays_inside` states the first rule: only `Normal` and `CurDir` components are accepted. `RepoScan` states the second: the scan owns the canonical repository root and answers `contains` for a directory by canonicalizing it and testing the prefix, which follows every link. Every directory the walk reads passes `contains`, so the check covers the whole file rather than the named line.

`RepoScan` also removes the reason the check had no home. The four walkers each threaded `packages: &mut Vec<_>` and `seen: &mut HashSet<_>`, and adding a `root: &Path` beside `dir: &Path` would have put two adjacent `&Path` parameters of different meaning into four signatures -- the newtype defect round 2 closed for `&str`. The struct carries the three together, so `scan_dir`, `scan_child_dirs`, and `scan_recursive` are methods with one parameter each, and the free-function `scan_recursive` is gone.

Four tests state the rule, all RED first against the old code, each failing by reading `outside-skill` from outside the repository: a `../` subpath, an absolute subpath, a subpath that is a symbolic link out, and a priority directory that is a symbolic link out. The last two are `#[cfg(unix)]`.

**The unnamed-tuple finding.** Two functions in `install/package.rs` returned an unnamed tuple of a name and a version, not one. `read_frontmatter` now returns `FrontmatterMetadata { name, version }`, and `parse_package_spec` now returns `PackageSpec { name, version }`. Twelve call sites read the named fields: two in `install/package.rs`, one in `install/uninstall.rs`, and eleven in `install/tests.rs`. No asserted value changed.

**The nesting finding.** `merge_packages` is a `match` at depth 2, and the inner target loop is `add_unique_targets`.

**The `rules/` literal finding.** The cause is a production literal that duplicates a constant which already names it, and `new.rs` carried five of them. `base_dir.join("rules")` now reads `package_type::VALIDATOR_RULES_DIR`, and the four manifest joins -- `SKILL.md`, `VALIDATOR.md`, `TOOL.md`, and `.claude-plugin/plugin.json` -- now read `PackageType::manifest_file()`. The plugin manifest join makes its own directory from `plugin_manifest.parent()`, so `.claude-plugin` is named once as well. The only production occurrence of `"rules"` left in the crate is the constant declaration itself; every other occurrence is inside a `mod tests`. The `println!` tree and the "Next steps" lines keep their literals: they are user-facing output, not paths, and the public-output-contract rule forbids rewording them.

**Verification.** `cargo nextest run --workspace`: 13633 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean. `cargo clippy -p mirdan --all-targets -- -W missing_docs`: zero findings in any file this change touched. `cargo doc -p mirdan --no-deps`: zero warnings in any file this change touched, including the pre-existing private-link warning in `git_source.rs`, which is now gone. `cargo fmt --all`.