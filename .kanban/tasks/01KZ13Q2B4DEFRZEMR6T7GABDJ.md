---
comments:
- actor: claude-code
  id: 01kzeb6ftdm3hhvzjncsym57v1
  text: |
    ### path correction before pickup

    This card names `crates/mirdan/src/install.rs`. That file no longer exists — `install` is now a directory module. Verified live sites, 2026-08-07:

    | card says | actual |
    |---|---|
    | `crates/mirdan/src/install.rs` | `crates/mirdan/src/install/applier.rs:443` |
    | `crates/mirdan/src/mcp_config.rs` | `crates/mirdan/src/mcp_config.rs:214` — confirmed |
    | `crates/mirdan/src/info.rs` | `crates/mirdan/src/info.rs:169` — confirmed |
    | `crates/mirdan/src/git_source.rs` | `crates/mirdan/src/git_source.rs:510` — confirmed |

    The shared splitter is `swissarmyhammer_common::frontmatter::split_frontmatter_body` at `crates/swissarmyhammer-common/src/frontmatter.rs:82`. `crates/mirdan/Cargo.toml:25` already depends on `swissarmyhammer-common`, so no manifest change is needed.
  timestamp: 2026-08-07T14:49:22.509387+00:00
- actor: claude-code
  id: 01kzebg9vdpzk6fcy4bpr1c4bg
  text: |-
    ### site map — corrected

    The file paths in the description are stale. `install.rs` is now the `install/` module. Verified locations:

    | function | file:line |
    |---|---|
    | `read_frontmatter` | `crates/mirdan/src/install/package.rs:307` |
    | `read_skill_frontmatter_name` | `crates/mirdan/src/install/uninstall.rs:347` — already delegates to `read_frontmatter`, so it is fixed by that site |
    | `frontmatter_map` (test helper) | `crates/mirdan/src/install/applier.rs:440` |
    | `parse_yaml_frontmatter` | `crates/mirdan/src/mcp_config.rs:202` |
    | `read_frontmatter_field` | `crates/mirdan/src/info.rs:157` |
    | `extract_name_from_frontmatter` | `crates/mirdan/src/git_source.rs:504` |

    Canonical splitter: `swissarmyhammer_common::frontmatter::split_frontmatter_body`, at `crates/swissarmyhammer-common/src/frontmatter.rs:55`.
  timestamp: 2026-08-07T14:54:44.077882+00:00
- actor: claude-code
  id: 01kzed04dkke1pe0q5cp7p56rz
  text: |-
    ### merged ^nk1yr64 into this card

    `^nk1yr64` was filed while implementing `^0zer2xf` and covers the same defect. It is archived; this card is the single home.

    Two corrections it contributes:

    - `crates/mirdan/src/install/applier.rs` calls `.expect("SKILL.md must open with frontmatter")`. A bad read panics. It must return an error instead. Record the chosen error on this card.
    - `crates/mirdan/src/install/package.rs:307` hides the delimiter behind a `FRONTMATTER_DELIMITER` constant, so a literal grep for `"---"` does not find it. It carries both faults.

    Full site list, five production functions plus one test helper:

    | file:line | function |
    |---|---|
    | `crates/mirdan/src/install/package.rs:307` | `read_frontmatter` |
    | `crates/mirdan/src/install/applier.rs:440` | `frontmatter_map` — test helper, panics |
    | `crates/mirdan/src/mcp_config.rs:202` | `parse_yaml_frontmatter` |
    | `crates/mirdan/src/info.rs:157` | `read_frontmatter_field` |
    | `crates/mirdan/src/git_source.rs:504` | `extract_name_from_frontmatter` |

    `crates/mirdan/src/install/uninstall.rs:347` `read_skill_frontmatter_name` delegates to `read_frontmatter`, so the `package.rs` fix covers it.

    Each file keeps its own error contract: `mcp_config.rs` returns `RegistryError::Validation`, `info.rs` returns `"unknown"`, `git_source.rs` returns `None`. Change only the split.
  timestamp: 2026-08-07T15:20:51.379268+00:00
- actor: claude-code
  id: 01kzeea6zf45dx525dqde0jq4d
  text: |-
    Satisfied by the work on ^0zer2xf. Read this before reviewing this card, and check the acceptance list against the code, not against a fresh implementation.

    Every site this card names now calls `swissarmyhammer_common::frontmatter::split_frontmatter_body`. Mirdan builds no frontmatter split of its own.

    The file layout moved since this card was written. `install.rs` is now the `install/` module, so the two functions this card places there live in two files:

    | this card says | the code is at | contract kept |
    |---|---|---|
    | `read_frontmatter` in `install.rs` | `crates/mirdan/src/install/package.rs` | `RegistryError::Validation`. The `FRONTMATTER_DELIMITER` constant is gone: the delimiter rule lives in the splitter now, not behind a name in this file. |
    | `read_skill_frontmatter_name` in `install.rs` | `crates/mirdan/src/install/uninstall.rs` | Unchanged code. It delegates to `read_frontmatter`, so it inherits the fix. The uninstall failure this card describes -- a mis-parsed name, `remove_dir_all` never runs, the skill stays installed -- is fixed through that delegation. |
    | `parse_yaml_frontmatter` in `mcp_config.rs` | same | `RegistryError::Validation` |
    | `read_frontmatter_field` in `info.rs` | same | `"unknown"` |
    | `extract_name_from_frontmatter` in `git_source.rs` | same | `None` |
    | `frontmatter_map`, test helper | `crates/mirdan/src/install/applier.rs` | Now returns `Result<serde_yaml_ng::Mapping, String>`. The `.expect(...)` calls are gone, and the two call sites name the file in the failure message, which the old `.expect` did not. Its old `find("\n---")` was anchored on the left only, so `\n---xyz` matched; that is gone with the rest. |

    ## Acceptance, item by item

    - **No frontmatter split of mirdan's own remains.** Confirmed: `rg 'strip_prefix\("---"\)' crates/mirdan/` returns nothing.
    - **RED first, one test per site.** Four tests for each of the five sites, twenty in all, against four shared fixtures. The RED run failed 12 of 16 for the four production sites, each for the predicted reason. The `applier.rs` tests could not compile against the old signature, because the `Result` contract is itself the change this card asks for.
    - **A file whose `description` block scalar holds a three-hyphen run keeps every frontmatter key.** The fixture is a real `description: >-` folded block scalar with an indented `---`, which is the construct this card warns about in the 21 builtin `VALIDATOR.md` and `SKILL.md` files. `name` and `metadata` sit *after* the block scalar, so a truncating split loses them and the loss is observable through a reader that exposes only one of the two.
    - **A first line of `----` or `---x` is not read as an opening delimiter.** Covered. The `----` case passed on the old code too, because the leftover `-` made the YAML unparseable. It is a pin, not a RED case.
    - **A file with no closing delimiter line is rejected.** Covered, including the hard case: an unterminated file whose value holds a `---` run, which the old split read as the close.
    - **No behavior change for well-formed input; existing tests pass unedited.** No pre-existing test was edited. `cargo nextest run --workspace`: 13607 passed, 0 failed, 0 skipped.

    ## Where the fixtures live

    `crates/mirdan/src/frontmatter_fixtures.rs`, a `#[cfg(test)]` module in the crate root. Four constants and one `write_skill_md` helper. Six sites read the same four files and assert their own error contract against them, so the delimiter rule is written once, not six times. A seventh reader should use these rather than write a fifth fixture.

    ## Coordination

    This card told whichever landed first not to add a second private helper. None was added. `crates/mirdan/src/list.rs` also calls the shared splitter, from work that landed earlier on ^0zer2xf.

    `^nk1yr64`, a duplicate follow-up card the earlier pass filed for these same copies, is archived.
  timestamp: 2026-08-07T15:43:50.255180+00:00
- actor: claude-code
  id: 01kzet8dt9s0f9wv6668myz9nx
  text: |-
    ### applier.rs error decision — the answer ^nk1yr64 asked for

    `^nk1yr64` asked what `crates/mirdan/src/install/applier.rs` returns in place of its `.expect("SKILL.md must open with frontmatter")` panic. The answer at HEAD:

    `frontmatter_map` returns `Result<serde_yaml_ng::Mapping, String>`. The error is a plain `String`, because the function is a `#[cfg(test)]` helper and has no production error type to carry. Two error strings:

    - Bad or absent delimiter: `"SKILL.md must open and close its frontmatter with a line of exactly three hyphens"`, from `split_frontmatter_body(md).ok_or(...)`.
    - YAML the parser rejects, which includes a duplicate key: `"frontmatter must be a YAML mapping with no duplicate keys: {e}"`.

    No `.expect` remains. Both call sites in `test_deployed_skills_keep_every_source_frontmatter_key` do `unwrap_or_else(|e| panic!("{source_md:?}: {e}"))`, so the failure names the file the bad frontmatter came from. The old `.expect` did not.
  timestamp: 2026-08-07T19:12:34.633411+00:00
- actor: claude-code
  id: 01kzet969mpp3p5gwp8kfg1fhy
  text: |-
    ### acceptance audit against HEAD, not against the resolution comment

    The earlier resolution comment (2026-08-07T15:43) was written before three more iterations landed on ^0zer2xf. Two things it says are no longer true of the code:

    - The fixtures moved. `crates/mirdan/src/frontmatter_fixtures.rs` is now `crates/mirdan/src/frontmatter/fixtures.rs`, a `#[cfg(test)]` child of the new `crates/mirdan/src/frontmatter.rs` module. That module is now the single reader: `parse`, `parse_file`, `read_file`, `field`, `metadata_field`, `file_field`, `file_metadata_field`. Every named site delegates to it.
    - `git_source::extract_name_from_frontmatter` is deleted as dead (commit `a95954c6c`). The resolution comment counts it as a covered site with a `None` contract. It no longer exists, and the four tests that covered it went with it. In its place `a95954c6c` left a comment saying the delimiter rule "are tested once, in `crate::frontmatter`".

    That deletion opened the one real gap.

    | acceptance item | holds at HEAD? |
    |---|---|
    | No frontmatter split of mirdan's own remains | Yes. `rg 'strip_prefix\("---"\)\|find\("---"\)\|find\("\\n---"\)' crates/mirdan/` returns only a doc comment in `fixtures.rs`. Every site calls `split_frontmatter_body`, directly or through `crate::frontmatter`. |
    | RED first, one test per site | Was NO for `git_source.rs`. Yes for the other four sites: `install/tests.rs` (4, `read_frontmatter`), `mcp_config.rs` (4), `info.rs` (4), `install/applier.rs` (4), plus 4 in `frontmatter.rs` itself. |
    | `----` or `---x` opens nothing | Same gap, git_source only. |
    | No closing delimiter is rejected | Same gap, git_source only. |
    | No behavior change for well-formed input; existing tests unedited | Yes. This pass edited no pre-existing test and no production line. |

    `read_skill_frontmatter_name` in `install/uninstall.rs` stays covered by delegation to `read_frontmatter`, as the merged `^nk1yr64` recorded. That reader takes input mirdan itself writes into its own store, so delegation is enough there.

    ### gap closed

    `git_source` is the one reader whose input this repository does not control -- a third-party repository writes the package files a scan walks. The delimiter rule must therefore hold on the discovery path itself, not only on the reader it delegates to. Four tests added to `crates/mirdan/src/git_source.rs`, against the production path `RepoScan::scan_dir` and the same four shared fixtures:

    - `test_scan_dir_keeps_every_key_past_a_three_hyphen_run`
    - `test_scan_dir_skips_a_skill_whose_opening_line_carries_trailing_text`
    - `test_scan_dir_skips_a_skill_whose_opening_line_is_four_hyphens`
    - `test_scan_dir_skips_a_skill_with_no_closing_delimiter`

    ### RED proved, not assumed

    The production code was already correct, so a new test passes on the first run and proves nothing. To show the tests bite, the pre-fix split from `60f6a1247^` (`starts_with("---")`, then `rest.find("---")`) was put back into `scan_dir`, the tests were run, and the production call was restored.

    RED: 3 of 4 failed, each for the predicted reason.

    - `keeps_every_key_past_a_three_hyphen_run`: 0 packages, not 1. The indented `---` in the `description` block scalar cut the frontmatter before `name`, so discovery found no name and skipped the package.
    - `skips_a_skill_whose_opening_line_carries_trailing_text`: not empty. `---description: leaked` was read as an opener, so a file that opens no block was discovered.
    - `skips_a_skill_with_no_closing_delimiter`: not empty. The `---` inside the value was read as the close, so a partial mapping parsed and gave a name.

    `skips_a_skill_whose_opening_line_is_four_hyphens` passed on the old code too. The leftover `-` makes the YAML unparseable, so it is a pin, not a RED case -- the same reading the earlier pass recorded for its own `----` case.

    GREEN after restore: 7 of 7 pass in `git_source::tests::test_scan_dir`.

    The comment `a95954c6c` left is replaced. It said the rule is tested once in `crate::frontmatter`; it now says why discovery is pinned as well.
  timestamp: 2026-08-07T19:12:59.700556+00:00
- actor: claude-code
  id: 01kzetjygdn8da1yenz2npnahw
  text: |-
    ### implement — changed
    - evidence: 1 code file — `crates/mirdan/src/git_source.rs` (+60/-3, tests only: 4 new tests, 1 fixtures import, 1 comment rewritten). No production line changed. Card description updated: `install.rs` -> `install/package.rs` and `install/uninstall.rs`, `frontmatter_map` -> `install/applier.rs`, `extract_name_from_frontmatter` -> `RepoScan::scan_dir`, acceptance item 1 -> `install/`.
    - RED: `cargo nextest run -p mirdan git_source::tests::test_scan_dir` against the reinstated pre-fix split — 7 run, 4 passed, 3 failed. GREEN after restore — 7 passed.
    - `cargo nextest run --workspace`: 13645 passed, 0 failed, 0 skipped.
    - `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean. `cargo fmt --all`: no change to the new code.
    - discovery, unrelated: the first workspace run failed `claude-agent collect_response_content_tests::a_lagged_collector_is_an_error_not_a_reply_with_holes`. It passes alone and it passed on the second full run. It is a load-sensitive flake in a broadcast-lag test, with no path to mirdan. It is not tracked here; file it separately if it repeats.
    - next: `/review`.
  timestamp: 2026-08-07T19:18:19.405908+00:00
position_column: doing
position_ordinal: '8380'
title: Four more mirdan frontmatter substring splits beyond list.rs
---
^0zer2xf covers `mirdan/src/list.rs` only. Four more production sites in `mirdan` carry the same bare `---` substring split.

File paths below are the paths that exist at HEAD. `install.rs` is now the `install/` module, and `git_source::extract_name_from_frontmatter` is deleted, so the discovery function that read through it names the site.

| function | file | what it reads |
|---|---|---|
| `read_frontmatter` | `crates/mirdan/src/install/package.rs` | `SKILL.md`, `VALIDATOR.md`, `TOOL.md`, `AGENT.md` on local package detect and on git-clone install |
| `read_skill_frontmatter_name` | `crates/mirdan/src/install/uninstall.rs` | `SKILL.md` in the nested skill store, for uninstall |
| `parse_yaml_frontmatter` | `crates/mirdan/src/mcp_config.rs` | `TOOL.md`, feeding `parse_tool_frontmatter` — the MCP server command, args and env |
| `read_frontmatter_field` | `crates/mirdan/src/info.rs` | `version` and `description` from `VALIDATOR.md` and `SKILL.md` |
| `RepoScan::scan_dir` | `crates/mirdan/src/git_source.rs` | package files found while scanning a cloned third-party git repository |

Each does `strip_prefix("---")` then `find("---")`. Both gates are weaker than the canonical `split_frontmatter_body`: the opener accepts `---anything`, and `find("---")` matches a three-hyphen run anywhere, including inside a block scalar.

## Why these matter more than the model-config copy

Twenty-one builtin `VALIDATOR.md` and `SKILL.md` files already write `description: >-` as a folded block scalar with indented multi-line prose. That is the exact construct in which a markdown table separator or a horizontal rule appears. The defect is one authored table away.

Failure is silent in every case:

- `read_frontmatter_field` returns `"unknown"`, so `mirdan info` prints a wrong version.
- `read_skill_frontmatter_name` mis-parses the name, `remove_dir_all` never runs, and uninstall leaves the skill installed.
- `RepoScan::scan_dir` makes the package invisible to discovery. This one reads third-party content, so it is fully outside this repository's control.
- `parse_yaml_frontmatter` reads `TOOL.md` command lines and args, where `--` and `---` runs are ordinary text.

## Also in the module, test scope only

`frontmatter_map` in the `#[cfg(test)]` module of `crates/mirdan/src/install/applier.rs` does `strip_prefix("---")` then `find("\n---")`. It is anchored on the left only: `\n---xyz` still matches. It reads no production document. Converge it with the others so the module holds one rule.

## Required change

Call `swissarmyhammer_common::frontmatter::split_frontmatter_body` at every site. Do not write another copy. `mirdan/Cargo.toml` already depends on `swissarmyhammer-common`, and there is no cycle.

Coordinate with ^0zer2xf: whichever card lands first, the other must not add a second private helper.

## Acceptance

- No frontmatter split of mirdan's own remains in `install/`, `mcp_config.rs`, `info.rs` or `git_source.rs`.
- RED first, one test per site: a package file whose `description` block scalar holds a three-hyphen run keeps every frontmatter key.
- A first line of `----` or `---x` is not read as an opening delimiter.
- A file with no closing delimiter line is rejected.
- No behavior change for well-formed input. Existing tests pass unedited.

Found by the workspace audit on ^8ax9jrq.

#bug