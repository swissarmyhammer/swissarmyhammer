---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8d80
title: mirdan carries four more copies of the frontmatter substring-split defect
---
`^0zer2xf` replaced the substring split in `crates/mirdan/src/list.rs` with `swissarmyhammer_common::frontmatter::split_frontmatter_body`. Four more copies of the same defect stay in the same crate.

## The copies

| File | Function | Shape |
|---|---|---|
| `crates/mirdan/src/install/applier.rs` | skill body rewrite | `.trim().strip_prefix("---").expect(...)` then `find` |
| `crates/mirdan/src/mcp_config.rs` | plugin/tool manifest read | `starts_with("---")`, `&content[3..]`, `rest.find("---")` |
| `crates/mirdan/src/info.rs` | version read for `mirdan info` | `starts_with("---")`, `&content[3..]`, `rest.find("---")` |
| `crates/mirdan/src/git_source.rs` | `extract_name_from_frontmatter` | `starts_with("---")`, `&content[3..]`, `rest.find("---")?` |

Each carries the two faults `^0zer2xf` names:

1. `rest.find("---")` finds the three-hyphen **substring** anywhere, not a delimiter line. A `---` run inside a description cuts the frontmatter short. The truncated text often still parses as valid YAML, so the read returns a partial mapping instead of failing.
2. `starts_with("---")` / `strip_prefix("---")` accepts a first line of `----` or `---x` as an opening delimiter.

`applier.rs` is the worst of the four: it `.expect("SKILL.md must open with frontmatter")`, so a bad read panics instead of returning an error.

## Required change

Call `swissarmyhammer_common::frontmatter::split_frontmatter_body` in each. `mirdan/Cargo.toml` already lists `swissarmyhammer-common`. Do not write a fifth copy.

Take the copies one file at a time. Each file keeps its own error contract: `mcp_config.rs` returns `RegistryError::Validation`, `info.rs` returns "unknown", `git_source.rs` returns `None`. Keep those; change only the split.

`applier.rs` must stop panicking. Decide the error it returns and record the decision on this card.

## Acceptance

- No file in `mirdan` builds a frontmatter split of its own.
- RED first, one test per file: a manifest whose `description` holds a three-hyphen run keeps every frontmatter key.
- RED first: a first line of `----` or `---x` is not read as an opening delimiter.
- `applier.rs` returns an error for a `SKILL.md` with no frontmatter. It does not panic.
- No behavior change for well-formed input: existing tests pass unedited.

Found while implementing `^0zer2xf`. Related: `^fpcbeth`, `^a2ef9wh`, `^tv3692e`. #bug