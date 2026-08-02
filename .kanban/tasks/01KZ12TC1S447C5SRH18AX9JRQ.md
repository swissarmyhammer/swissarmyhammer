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
- actor: wballard
  id: 01kz13s9gsp5ab9f2nyeh5qj2w
  text: |-
    Implementation landed.

    ## The test

    `crates/swissarmyhammer-entity/src/io.rs` now holds `a_card_whose_comment_holds_a_markdown_table_reads_every_field`, with the card bytes in the `P4MP9N6_CARD` const. It writes those bytes to a temp file and calls `read_entity`, which is the function the kanban tool reads a card with. It asserts the title, the column, the ordinal, that the body does not start with `|---|`, that the body is the real description, and that the table separator stays inside the comment it belongs to.

    This is the first read-side test in the file. Every other delimiter test is a round trip, and a round trip cannot catch this class of defect on its own, because the writer and the reader agree with each other.

    ## RED, then GREEN

    RED: I put the pre-42e32c3a3 body back into `parse_frontmatter_body`.

    ```
    test io::tests::a_card_whose_comment_holds_a_markdown_table_reads_every_field ... FAILED
    assertion `left == right` failed
      left: None
     right: Some("Lowercase the remaining capitalized MCP error Display messages outside the kanban tool")
    ```

    `title` is `None`, the same symptom the CLI showed.

    GREEN after the revert to `split_frontmatter_body`. `git diff` is insertions only. No production code changed.

    ## The remedy for the CLI

    `cargo install --locked --path apps/swissarmyhammer-cli` from this worktree. Cargo printed the proof of the root cause itself:

    ```
    Replaced package `swissarmyhammer-cli v0.17.0 (/Users/wballard/github/swissarmyhammer/swissarmyhammer/apps/swissarmyhammer-cli)`
       with `swissarmyhammer-cli v0.17.0 (/Users/wballard/github/swissarmyhammer/swissarmyhammer-main/apps/swissarmyhammer-cli)` (executable `sah`)
    ```

    The installed binary came from the `review` worktree, which does not hold `42e32c3a3`.

    All four cards now read correctly through `~/.cargo/bin/sah`. No `.md` file was edited.

    ## Verification

    - `cargo fmt --all` clean
    - `cargo clippy --workspace --all-targets -- -D warnings` clean, zero warnings
    - `cargo nextest run -p swissarmyhammer-common -p swissarmyhammer-entity -p swissarmyhammer-kanban`: 2552 tests run, 2552 passed, 0 skipped

    ## New work found, all carded

    A workspace audit found nine more production sites that split frontmatter on their own, beyond the one `^0zer2xf` already holds. New cards:

    - `^tqbwr5v` — three copies in `swissarmyhammer-config/src/model.rs`
    - `^t7gabdj` — four more in `mirdan`, beyond `list.rs`
    - `^dkd510v` — `skill_loader.rs` `split_frontmatter`, which reads every `SKILL.md`
    - `^45yhyvp` — `swissarmyhammer-templating`, which trims the closer and miscounts CRLF

    `swissarmyhammer-merge::frontmatter::split_frontmatter` uses the correct delimiter rule and has no caller in the workspace. It is not byte-preserving, but it needs no card for this defect.
  timestamp: 2026-08-02T11:29:42.425352+00:00
- actor: wballard
  id: 01kz140ha7f09zpf7p40qxem7v
  text: |-
    Note for the next agent: a parallel shell committed this work while I was running it.

    ## What happened

    `main` moved under me from `6faad6a40` to `4dd1b818d` mid-task. The test I wrote in my working tree is gone. An equivalent test landed instead:

    - `ace417d75 test(entity): pin that a markdown table in a comment must not truncate frontmatter`
    - `13884f057 fix(entity): drop orphan #[test] left by the regression test insertion`
    - `66f9bb975 chore(kanban): file frontmatter substring-split follow-up cards, root-cause note on stale binary` — this one carries all five of my cards
    - `4dd1b818d Merge branch 'review'`

    ## The test of record

    `a_markdown_table_in_a_comment_does_not_truncate_the_frontmatter` in `crates/swissarmyhammer-entity/src/io.rs`. It calls `parse_frontmatter_body` directly with a fixture reduced from the real card, and asserts `title`, `position_column`, `position_ordinal`, and that the body does not start with `|---|`.

    I did not trust the RED claim in the commit message. I proved it myself: I put the pre-42e32c3a3 substring split back into `parse_frontmatter_body` and ran the committed test.

    ```
    test io::tests::a_markdown_table_in_a_comment_does_not_truncate_the_frontmatter ... FAILED
    assertion `left == right` failed: the title is written after the comment; a substring split loses it
      left: None
     right: Some("Lowercase the remaining capitalized MCP error Display messages")
    ```

    Reverted with `git checkout`, and the test is green again. `crates/` is clean.

    ## Why I did not re-add my version

    My version went through `read_entity` rather than `parse_frontmatter_body`. The delta between the two is a file read and a branch on `entity_def.body_field`, and `read_write_entity_with_body_field` already covers that pair. A second test of the same fixture would add no defect it can catch that the committed test cannot. One test of record is enough.

    ## Verification at 4dd1b818d

    - `cargo fmt --all` clean, no diff
    - `cargo clippy --workspace --all-targets -- -D warnings` clean, zero warnings
    - `cargo nextest run -p swissarmyhammer-common -p swissarmyhammer-entity -p swissarmyhammer-kanban`: 2552 tests run, 2552 passed, 0 skipped
    - All four cards read their true title, column and ordinal through `~/.cargo/bin/sah`, and the description of `^p4mp9n6` starts at its real first line, not at `|---|`
  timestamp: 2026-08-02T11:33:39.783322+00:00
- actor: wballard
  id: 01kz14g1xd5e8gnb7ha1wvyw02
  text: |-
    Double-check verdict: REVISE, with two findings. Both are done.

    ## Finding 1

    "The test passes the wrong body-field name — `description` is not the task body field."

    Location: `crates/swissarmyhammer-entity/src/io.rs`, `a_markdown_table_in_a_comment_does_not_truncate_the_frontmatter` — the `"description"` argument to `parse_frontmatter_body` and the matching `parsed.get_str("description")`.

    Problem: "The real `task` entity's body field is `"body"`, not `"description"`. `crates/swissarmyhammer-kanban/src/defaults.rs:787` asserts `entity.body_field == Some("body".into())`, and the sibling helper `task_entity_def()` in this very test module declares `body_field: Some("body".into())`. The test's doc comment presents it as a reduction of the real card and the real read path, so it embeds a factual error about that path. `parse_frontmatter_body` treats the argument as an opaque map key, so the defect coverage is unaffected — but a reader will conclude `description` is the task body field, and it is not."

    I confirmed it before acting. `crates/swissarmyhammer-kanban/builtin/entities/task.yaml` declares `body_field: body`. `description` is an API-layer rename only: `crates/swissarmyhammer-kanban/src/task_helpers.rs` documents `"body" -> "description" (rename for backward compat)` and writes `"description": entity.get_str("body")`.

    Fixed. Both occurrences now read `"body"`. I checked the whole file for the same cause: `"description"` appears nowhere else in `io.rs`.

    The test changed, so I proved it again from both sides.

    GREEN with the line-anchored split.

    RED with the pre-42e32c3a3 substring split put back:

    ```
    test io::tests::a_markdown_table_in_a_comment_does_not_truncate_the_frontmatter ... FAILED
    assertion `left == right` failed: the title is written after the comment; a substring split loses it
      left: None
     right: Some("Lowercase the remaining capitalized MCP error Display messages")
    ```

    ## Finding 2

    "The 'green' claim preceding commit `ace417d75` was not backed by a real run."

    Location: commit `ace417d75`, `crates/swissarmyhammer-entity/src/io.rs` — the duplicated `#[test]` later removed by `13884f057`.

    Problem: "`ace417d75` shipped two `#[test]` attributes on one fn. I reproduced with `rustc --test` that this emits `warning: duplicated attribute` under the default-on `duplicate_macro_attributes` lint. This repo's standard gate is `cargo clippy --all-targets -- -D warnings` (cited in the acceptance criteria of the very cards in this batch), which turns that warning into a hard failure. A clean clippy run therefore could not have been performed before that commit, yet the work was committed as verified. It took follow-up commit `13884f057` to repair. The defect is fixed at HEAD; the verification discipline that let it land is the finding."

    Suggested fix: "Run `cargo clippy --workspace --all-targets -- -D warnings` and capture its exit code before committing test insertions, and quote that evidence in the commit or task note rather than asserting green."

    `ace417d75` was made by a parallel shell, not by me. The code defect is already repaired at HEAD, and I verified independently that the repair is complete: I scanned all 64 functions in the `io.rs` tests module for a missing or duplicated test attribute. The only functions without one are the four real helpers — `task_entity_def`, `tag_entity_def`, `task_with_comment`, `assert_frontmatter_round_trip`. No test lost its attribute and none carries two.

    Acting on the process half: exit codes, captured, not asserted.

    ```
    cargo fmt --all                                           -> clean, no diff
    cargo clippy --workspace --all-targets -- -D warnings     -> exit 0, 0 warnings, 0 errors
    cargo nextest run -p swissarmyhammer-common \
      -p swissarmyhammer-entity -p swissarmyhammer-kanban     -> exit 0, 2552 run, 2552 passed, 0 skipped
    ```

    ## Confirmed by the double-check, not disputed

    - The provenance root cause is proven. `42e32c3a3` is not an ancestor of `c5a5e7ae5`, and `git show c5a5e7ae5:crates/swissarmyhammer-entity/src/io.rs` still holds `content.splitn(3, "---")`.
    - The fixture is a faithful reduction of the real card.
    - All four `.md` files are correct on disk. Not repairing them was right.
    - The shortened title in the fixture does not weaken the test. What carries the load is that `title:` sits after the table row.
    - Leaving out a `read_entity`-level test is defensible. `read_entity` with `task_entity_def()` is already covered three times in the file, and both kanban read paths route through the line-anchored splitter: `io.rs` `parse_frontmatter_body`, and `crates/swissarmyhammer-entity/src/store.rs` `EntityTypeStore::deserialize`, which backs `StoreHandle::get` and every changelog replay.
    - Provenance is the whole story for this card. No independent splitter reads `.kanban` task files.

    ## State

    The two-line body-field fix is uncommitted in the working tree. Everything else in this card is already committed.
  timestamp: 2026-08-02T11:42:08.301673+00:00
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