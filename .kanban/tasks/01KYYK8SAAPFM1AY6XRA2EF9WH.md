---
assignees:
- claude-code
position_column: todo
position_ordinal: d780
title: Two more copies of the frontmatter substring-split bug in swissarmyhammer-tools
---
The same defect fixed for the entity layer in ^fpcbeth survives in two more places, both in `swissarmyhammer-tools`. Each splits frontmatter on the bare three-hyphen **substring** instead of a line-anchored delimiter, so any occurrence inside the frontmatter block truncates the parse and silently drops every key after it.

## The two sites

`crates/swissarmyhammer-tools/src/health_registry.rs:101`

```rust
if content.starts_with("---") {
    let parts: Vec<&str> = content.splitn(3, "---").collect();
```

`crates/swissarmyhammer-tools/src/mcp/tools/ralph/state.rs:183`

```rust
fn parse_ralph_file(content: &str) -> Option<RalphState> {
    // Split on frontmatter delimiters
    let parts: Vec<&str> = content.splitn(3, "---").collect();
```

Found by the blast-radius sweep while implementing ^fpcbeth. Left out of that task deliberately: different crate, different files, so they were split rather than folded in.

## Why this is not merely cosmetic

^fpcbeth is not a theoretical bug. It fired three separate times on real data during the work that found it, each time silently blanking a card's `title` and dropping it off its board column, with the on-disk bytes intact so the damage only appeared on the next write. `parse_ralph_file` reads agent-authored state, and `health_registry` reads file content of the same shape. Both are exposed to the same free-form prose that triggered it.

Note `health_registry.rs:100` also gates on `content.starts_with("---")`, which accepts a file beginning `----` or `---x` as valid frontmatter.

## Required change

Use the shared splitter rather than re-deriving the parse a third time. ^fpcbeth added `split_frontmatter_body(content)` in `crates/swissarmyhammer-entity/src/frontmatter.rs`, which returns the frontmatter and body as borrowed slices, anchors both delimiters to whole lines, tolerates a trailing carriage return, and returns `None` for invalid frontmatter.

If `swissarmyhammer-tools` should not depend on `swissarmyhammer-entity` for this, promote the splitter to a crate both can use — but do not write a third copy of the logic.

## Acceptance

- Neither file constructs its own frontmatter split; both go through one shared implementation.
- A ralph state file whose instruction text contains a bare three-hyphen line on its own line round-trips with every field intact. Prove it RED first.
- The `starts_with` gate no longer accepts a first line that merely begins with three hyphens.
- A malformed file with no closing delimiter is still rejected, not treated as all-frontmatter.
- No behavior change for well-formed input: existing tests pass unedited.

Blocked by nothing, but do it after ^fpcbeth lands so the shared splitter exists to call. #bug