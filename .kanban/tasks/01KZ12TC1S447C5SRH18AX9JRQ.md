---
comments:
- actor: wballard
  id: 01kz130yjcpn5zyx9q49dy81ma
  text: |-
    Picked up. Research done. Root cause found, and it is not the code on `main`.

    ## The four cards are correct on disk

    Each of the four `.md` files has exactly two lines that are three hyphens at column 0, and holds `position_column`, `position_ordinal` and `title` at the end of the frontmatter. No file needs repair.

    ## The read path on `main` is already correct

    `crates/swissarmyhammer-entity/src/io.rs` `parse_frontmatter_body` calls the line-anchored `split_frontmatter_body`. That is the function `read_entity` uses for every `.md` entity, and it is what the kanban tool reads a card with. Commit `42e32c3a3` made that change and is an ancestor of `main`.

    ## The installed binary is stale

    The observed failure comes from `/Users/wballard/.cargo/bin/sah`, built at 2026-08-02 05:58 from a worktree that does not hold `42e32c3a3`. The sibling worktree on branch `review` at `c5a5e7ae5` does not hold it, and `split_frontmatter_body` does not exist in that tree.

    Same board, same files, two binaries:

    ```
    $ ~/.cargo/bin/sah tool kanban --cwd <main> task get --id 01KYT6GXEAP93V439V2P4MP9N6
      column: ''
      ordinal: '80'
    title: ''

    $ ./target/debug/sah tool kanban --cwd <main> task get --id 01KYT6GXEAP93V439V2P4MP9N6
      column: review
      ordinal: '8280'
    title: Lowercase the remaining capitalized MCP error Display messages outside the kanban tool
    ```

    All four cards read correctly with the binary built from `main`.

    ## The cache is not the cause

    `.kanban/search-cache.sqlite3` is present, but both binaries read the same board with that same cache file, and only one of them reads wrong. The cache does not explain the difference.

    ## The coverage gap that let this ship

    Every delimiter test in `io.rs` is a round trip: it builds an `Entity`, calls `format_frontmatter_body`, then reads the result back. A round trip cannot catch this class of defect on its own, because the writer and the reader agree with each other. No test read a literal card file. That is the gap this card closes.
  timestamp: 2026-08-02T11:16:24.780431+00:00
position_column: doing
position_ordinal: '8280'
title: Regression test that a real kanban card with a markdown table in a comment parses
---
`sah tool kanban task get` reads four cards on this board with an empty `title` and an empty `position.column`. The ordinal falls back to `'80'`, and the description starts at the literal text `|---|`.

Reproduce it:

```
sah tool kanban --cwd /Users/wballard/github/swissarmyhammer/swissarmyhammer-main task get --id 01KYT6GXEAP93V439V2P4MP9N6
```

## The four cards

- `01KYT6GXEAP93V439V2P4MP9N6` (^p4mp9n6)
- `01KYYK8SAAPFM1AY6XRA2EF9WH`
- `01KYYN9JQVYVXXPHS4AJ4D2613`
- `01KYYSBQZ3RC6WDMQY6TV3692E` (^tv3692e)

## The files on disk are correct

Each of the four `.md` files has exactly two lines that are three hyphens at column 0: line 1 and the closing line. `position_column`, `position_ordinal` and `title` are all present at the end of the frontmatter, after a long `comments:` block.

Example, `01KYT6GXEAP93V439V2P4MP9N6.md`:

- line 1 `---`, line 103 `---`
- line 100 `position_column: review`
- line 101 `position_ordinal: '8280'`
- line 102 `title: Lowercase the remaining capitalized MCP error Display messages outside the kanban tool`
- line 13 `    |---|---|`, a markdown table separator inside a comment block scalar

Do not repair the files. The parser reads them incorrectly. The bytes are good.

## Cause

The old `parse_frontmatter_body` in `crates/swissarmyhammer-entity/src/io.rs` split on the bare substring:

```rust
let parts: Vec<&str> = content.splitn(3, "---").collect();
```

That substring matches inside `    |---|---|` at line 13. The frontmatter stops there, the remainder `|---|` becomes the description, and the three fields at the end of the frontmatter are never read.

Commit `42e32c3a3 fix(entity): parse frontmatter on line boundaries, not substring` replaced that split with the line-anchored `split_frontmatter_body` in `crates/swissarmyhammer-common/src/frontmatter.rs`. `42e32c3a3` is an ancestor of `main`.

## The source on main is correct. The installed binary is not.

The installed `/Users/wballard/.cargo/bin/sah` was built from a worktree that does not hold `42e32c3a3`. The sibling worktree on branch `review` at `c5a5e7ae5` does not hold that commit, and `split_frontmatter_body` does not exist there.

Proof:

| binary | ^p4mp9n6 title | column | ordinal |
|---|---|---|---|
| `~/.cargo/bin/sah` | empty | empty | `80` |
| `target/debug/sah` built from `main` | correct | `review` | `8280` |

All four cards read correctly with the binary built from `main`.

## Work

1. Add a read-side regression test on the real card bytes. Every test in `io.rs` today is a round trip that formats an entity and reads it back. None reads a literal card file. Add a fixture taken from `01KYT6GXEAP93V439V2P4MP9N6.md` and assert `title`, `position_column` and `position_ordinal` all read their true values, and that the body does not start with `|---|`.
2. Prove the test is red against the substring split and green against the line-anchored split.
3. Install `sah` from `main` so the CLI holds the fix.

## Not in scope

`parse_frontmatter` in `swissarmyhammer-common` still splits on the `---\n` substring. ^tv3692e owns that. The bare `find("---")` copies in `swissarmyhammer-config::model` and in `mirdan` are owned by ^0zer2xf.

#bug