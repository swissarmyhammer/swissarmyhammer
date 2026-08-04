---
comments:
- actor: claude-code
  id: 01kz7c534q1e7tmv099psp92n8
  text: |
    ## Research

    Read the reference fix at `crates/swissarmyhammer-entity/src/io.rs` and the canonical splitter at `crates/swissarmyhammer-common/src/frontmatter.rs::split_frontmatter_body`. Confirmed via `git log` that the third function named in the task description, `extract_yaml_frontmatter_list` (called by a `parse_model_tags`), no longer exists in `crates/swissarmyhammer-config/src/model.rs` — a prior refactor (`196454ed1 refactor(config): dedupe model loading, lowercase error messages`) removed it. Only two substring frontmatter splits remain in the file: `extract_yaml_frontmatter_field` and `parse_model_config`. Both were fixed.

    ## Implementation

    - Added `use swissarmyhammer_common::frontmatter::split_frontmatter_body;` to the imports.
    - `extract_yaml_frontmatter_field`: replaced `content.strip_prefix("---")` + `stripped.find("---")` with a single call to `split_frontmatter_body(content)?`, taking the returned `front_matter` slice.
    - `parse_model_config`: replaced the same substring pair (plus the `&stripped[end_pos + 3..]` remainder slice) with `split_frontmatter_body(content)`, parsing `body.trim()` as the `ModelConfig` YAML.
    - `model.rs` now builds no frontmatter split of its own.

    ## TDD

    RED first, per the acceptance criteria: added `test_parse_model_description_survives_triple_hyphen_in_value` and `test_parse_agent_config_survives_triple_hyphen_in_frontmatter_value`, both using a `description: "Claude Code --- installed separately"` frontmatter value. Ran `cargo test -p swissarmyhammer-config --lib model::tests` before the fix — confirmed exactly these two tests FAILED (old code cut the YAML mid-scalar, `serde_yaml_ng` failed, and both functions silently lost the frontmatter). Also added three regression tests for the other acceptance bullets (`----` opening line, `---x` opening line, missing closing delimiter) — these already passed under the old code (parity, not RED), which is expected since those miscuts happen to break YAML syntax anyway.

    After the fix: all 5 new tests pass, `cargo build -p swissarmyhammer-config` is clean, `cargo test -p swissarmyhammer-config --lib model::tests` is 142 passed / 0 failed, full `cargo test -p swissarmyhammer-config` (unit + doctests) is green with 0 failures, `cargo clippy -p swissarmyhammer-config --lib --tests` has 0 warnings, and `mcp__sah__diagnostics check working` reports 0 errors / 0 warnings. No existing test was edited.

    ### implement — changed
    - evidence: 1 file — `crates/swissarmyhammer-config/src/model.rs` (added `split_frontmatter_body` import, rewrote `extract_yaml_frontmatter_field` and `parse_model_config` to delegate to it, added 5 tests: 2 RED-first regression tests for the triple-hyphen-in-value bug, 3 additional regression tests for malformed-opener/missing-closer cases)
    - next: ready for review
  timestamp: 2026-08-04T21:51:24.311814+00:00
- actor: claude-code
  id: 01kz7cb9kd0bt2pd3tabkaw88h
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace — 13506 tests run, 13506 passed, 0 failed, 0 skipped. cargo clippy --workspace --all-targets --all-features -- -D warnings — 0 warnings.
    - next: none, no fixes needed.
  timestamp: 2026-08-04T21:54:47.533833+00:00
position_column: doing
position_ordinal: '8380'
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