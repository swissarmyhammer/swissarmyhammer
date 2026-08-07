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