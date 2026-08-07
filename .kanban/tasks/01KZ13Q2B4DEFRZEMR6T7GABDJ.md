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
position_column: todo
position_ordinal: e480
title: Four more mirdan frontmatter substring splits beyond list.rs
---
^0zer2xf covers `mirdan/src/list.rs` only. Four more production sites in `mirdan` carry the same bare `---` substring split.

| function | file | what it reads |
|---|---|---|
| `read_frontmatter` | `crates/mirdan/src/install.rs` | `SKILL.md`, `VALIDATOR.md`, `TOOL.md`, `AGENT.md` on local package detect and on git-clone install |
| `read_skill_frontmatter_name` | `crates/mirdan/src/install.rs` | `SKILL.md` in the nested skill store, for uninstall |
| `parse_yaml_frontmatter` | `crates/mirdan/src/mcp_config.rs` | `TOOL.md`, feeding `parse_tool_frontmatter` — the MCP server command, args and env |
| `read_frontmatter_field` | `crates/mirdan/src/info.rs` | `version` and `description` from `VALIDATOR.md` and `SKILL.md` |
| `extract_name_from_frontmatter` | `crates/mirdan/src/git_source.rs` | package files found while scanning a cloned third-party git repository |

Each does `strip_prefix("---")` then `find("---")`. Both gates are weaker than the canonical `split_frontmatter_body`: the opener accepts `---anything`, and `find("---")` matches a three-hyphen run anywhere, including inside a block scalar.

## Why these matter more than the model-config copy

Twenty-one builtin `VALIDATOR.md` and `SKILL.md` files already write `description: >-` as a folded block scalar with indented multi-line prose. That is the exact construct in which a markdown table separator or a horizontal rule appears. The defect is one authored table away.

Failure is silent in every case:

- `read_frontmatter_field` returns `"unknown"`, so `mirdan info` prints a wrong version.
- `read_skill_frontmatter_name` mis-parses the name, `remove_dir_all` never runs, and uninstall leaves the skill installed.
- `extract_name_from_frontmatter` makes the package invisible to discovery. This one reads third-party content, so it is fully outside this repository's control.
- `parse_yaml_frontmatter` reads `TOOL.md` command lines and args, where `--` and `---` runs are ordinary text.

## Also in the file, test scope only

`frontmatter_map` in the `#[cfg(test)]` module of `install.rs` does `strip_prefix("---")` then `find("\n---")`. It is anchored on the left only: `\n---xyz` still matches. It reads no production document. Converge it with the others so the file holds one rule.

## Required change

Call `swissarmyhammer_common::frontmatter::split_frontmatter_body` at every site. Do not write another copy. `mirdan/Cargo.toml` already depends on `swissarmyhammer-common`, and there is no cycle.

Coordinate with ^0zer2xf: whichever card lands first, the other must not add a second private helper.

## Acceptance

- No frontmatter split of mirdan's own remains in `install.rs`, `mcp_config.rs`, `info.rs` or `git_source.rs`.
- RED first, one test per site: a package file whose `description` block scalar holds a three-hyphen run keeps every frontmatter key.
- A first line of `----` or `---x` is not read as an opening delimiter.
- A file with no closing delimiter line is rejected.
- No behavior change for well-formed input. Existing tests pass unedited.

Found by the workspace audit on ^8ax9jrq.

#bug