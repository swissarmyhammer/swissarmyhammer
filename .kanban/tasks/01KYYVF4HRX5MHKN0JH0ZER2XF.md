---
assignees:
- claude-code
position_column: todo
position_ordinal: db80
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