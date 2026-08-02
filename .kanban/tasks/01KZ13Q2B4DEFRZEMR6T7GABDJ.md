---
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