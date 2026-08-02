---
position_column: todo
position_ordinal: e380
title: swissarmyhammer-config model.rs carries three substring frontmatter splits
---
`crates/swissarmyhammer-config/src/model.rs` carries three private frontmatter splits that cut on the bare `---` substring:

- `extract_yaml_frontmatter_list` — `content.strip_prefix("---")` then `stripped.find("---")`
- `extract_yaml_frontmatter_field` — the same pair
- `parse_model_config` — the same pair, plus `&stripped[end_pos + 3..]` for the remainder

## What they read

Model configuration files: `builtin/models/*.yaml`, plus the project and user model directories. `parse_model_tags` and `parse_model_description` call the first two. `parse_model_config` is called from five sites in the same file.

## Why it is wrong

Both gates are weaker than the canonical splitter:

1. The opener accepts `---anything` with no newline after the three hyphens.
2. `find("---")` matches a three-hyphen run on the same line as a value. A description such as `Claude Code --- installed separately` cuts the frontmatter in the middle of that scalar.

`parse_model_config` has the worst result. The slice starts in the middle of a value, `serde_yaml_ng` fails, and `validate_and_create_model_info` drops the whole model with only a `tracing::warn!`. The user sees a model disappear.

The other two fail silently: the YAML parse fails, the function returns `None`, and the description or the tag list is lost.

Builtin model files use single-line quoted scalars today, so the defect is latent there. Project and user model files are not controlled.

## Required change

Call `swissarmyhammer_common::frontmatter::split_frontmatter_body`. It is line-anchored, it accepts CRLF, and it returns `None` for a malformed block. Do not write another copy.

`swissarmyhammer-config/Cargo.toml` already depends on `swissarmyhammer-common`. `swissarmyhammer-common` does not depend on `swissarmyhammer-config`, so there is no cycle.

## Acceptance

- `model.rs` builds no frontmatter split of its own.
- RED first: a model file whose `description` holds a three-hyphen run keeps every frontmatter key, and the model still loads.
- A first line of `----` or `---x` is not read as an opening delimiter.
- A file with no closing delimiter line is rejected.
- No behavior change for well-formed input. Existing tests pass unedited.

Found by the workspace audit on ^8ax9jrq. Related: ^tv3692e, ^0zer2xf, ^a2ef9wh.

#bug