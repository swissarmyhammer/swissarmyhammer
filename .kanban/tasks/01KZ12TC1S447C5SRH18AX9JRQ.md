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
- actor: claude-code
  id: 01kz8vza1fa7nw7rdemppn1fw1
  text: |-
    ### review — findings
    - evidence: 5 findings — crates/swissarmyhammer-entity/src/io.rs:103, crates/swissarmyhammer-entity/src/io.rs:251, crates/swissarmyhammer-entity/src/io.rs:280, crates/swissarmyhammer-entity/src/io.rs:376, crates/swissarmyhammer-entity/src/io.rs:381
    - next: address the panic-documentation and duplication findings in crates/swissarmyhammer-entity/src/io.rs, then re-run /review 01KZ12TC1S447C5SRH18AX9JRQ
  timestamp: 2026-08-05T11:47:06.415683+00:00
- actor: claude-code
  id: 01kz8w7xv6arve199zt0xwy8ye
  text: |
    ### implement — changed
    - evidence: crates/swissarmyhammer-entity/src/io.rs — added `# Panics` doc sections to `write_entity`, `trash_entity_files`, `restore_entity_files`; extracted `yaml_error(path, e)` helper and replaced all 4 verbatim `.map_err(|e| EntityError::Yaml {...})` call sites; extracted `build_entity_from_yaml(entity_type, id, yaml_map)` helper and replaced the duplicated entity-construction block in `parse_frontmatter_body` and `parse_plain_yaml`. All 5 review findings checked `[x]` on the card.
    - verification: `cargo fmt --all` exit 0 (no diff); `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings; `cargo nextest run -p swissarmyhammer-entity` exit 0, 327 tests run, 327 passed, 0 skipped
    - next: ready for /review
  timestamp: 2026-08-05T11:51:48.838727+00:00
- actor: claude-code
  id: 01kz8wr6j8yv1bnjsazxv7majj
  text: |-
    ### review — findings
    - evidence: `review sha b12b1b630~1..b12b1b630` returned 5 confirmed findings. 3 (io.rs:1258, io.rs:1335, io.rs:1535) target magic numbers inside pre-existing test functions untouched by this diff — dropped per the never-refactor-existing-tests rule. 2 stand: io.rs:122 (write_entity panics on missing parent dir instead of returning Result), io.rs:300 (restore_entity_files panics on missing filename instead of returning Result).
    - next: address the two panic-vs-Result findings, then re-review scoped to the fix commit.
  timestamp: 2026-08-05T12:00:42.056109+00:00
- actor: claude-code
  id: 01kz8wrykt76vhmq2q6r36zqhj
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — crates/swissarmyhammer-entity/src/io.rs (39 insertions, 24 deletions); fixed 5 prior findings (undocumented panics, duplicated YAML helpers)
    - test: green — cargo nextest run -p swissarmyhammer-entity, 327 passed, 0 skipped
    - commit: b12b1b630
    - review: findings — crates/swissarmyhammer-entity/src/io.rs:122 (write_entity panics via .expect() instead of returning Result), crates/swissarmyhammer-entity/src/io.rs:300 (restore_entity_files panics via .expect() instead of returning Result)
  timestamp: 2026-08-05T12:01:06.682260+00:00
- actor: claude-code
  id: 01kz9113g2jdj3vsaw5yjfbz2y
  text: |-
    ### implement — changed

    Fixed both open findings: `write_entity` and `restore_entity_files` no longer panic via `.expect()` on structurally invalid paths — both return the new `EntityError::InvalidPath { path, reason }` variant instead.

    ## Core change

    `crates/swissarmyhammer-entity/src/error.rs` — added `EntityError::InvalidPath { path: PathBuf, reason: String }`, `#[error("invalid path {path}: {reason}")]`.

    `crates/swissarmyhammer-entity/src/io.rs`:
    - `write_entity` — `path.parent().expect(...)` → `require_parent(path)?`; also added a `require_filename(path)?` check (see completeness note below).
    - `trash_entity_files` — both `.expect("...filename")` → `require_filename(path)?`.
    - `restore_entity_files` — `path.parent().expect(...)` → `require_parent(path)?` (was previously a tolerant `if let Some`); both filename `.expect()` → `require_filename(path)?`.
    - New shared helpers: `invalid_path_error(path, reason)`, `require_parent(path) -> Result<&Path>`, `require_filename(path) -> Result<&OsStr>`, `temp_file_path(dir) -> PathBuf`, `changelog_trash_paths(path, trash_dir)`, `move_changelog_to_trash`, `move_changelog_from_trash`.
    - `apps/kanban-cli/src/commands/serve.rs` `classify_entity_error_kind` — added the new `EntityError::InvalidPath` arm (compile-time exhaustiveness caught this caller), classified `InvalidRequest` (caller-addressable per self-review finding, see below).

    ## Self-review rounds (findings are requirements — all confirmed findings about code this diff touches were fixed)

    Round 1 (io.rs:122, io.rs:300 — the two assigned findings): fixed as above.

    Round 2 (4 new confirmed findings, all about the code I'd just written): fixed all —
    - `missing_parent_error`/`missing_filename_error` deduplicated into `invalid_path_error(path, reason)` + `require_parent`/`require_filename`.
    - Completeness: `write_entity` validated parent but not filename, unlike the trash/restore siblings — added `require_filename(path)?` to `write_entity` too. New test `write_entity_errors_when_path_has_no_filename` uses `Path::new(".")`, verified via `rustc` probe and the passing test itself that `.` has `parent = Some("")`, `file_name = None` — the exact case this closes (parent exists, filename doesn't; previously this would have failed later at the OS-level rename with a generic `Io` error instead of a typed one).
    - `restore_entity_files` hardened from tolerant `if let Some(parent)` to `require_parent(path)?`, matching `write_entity`.
    - `trash_entity_files`/`restore_entity_files` changelog-path duplication collapsed into `changelog_trash_paths`.
    - Added documenting test `read_entity_returns_io_error_when_path_has_no_filename` for the one finding that offered a non-behavior-change alternative (asymmetry between `read_entity` and the write-side validation is intentional — `read_entity` never constructs a second path from `path`, so there's no arithmetic step needing a precondition).

    Round 3 (4 new confirmed findings): fixed all —
    - `temp_file_path(dir)` extracted, deduplicating `write_entity` and the pre-existing `copy_attachment` (same expression, untouched by me until now).
    - `changelog_trash_paths` further split into `move_changelog_to_trash`/`move_changelog_from_trash` (one of the two options the finding itself offered).
    - `EntityError::InvalidPath` reclassified `Internal` → `InvalidRequest` in `serve.rs` per the finding's reasoning (caller-addressable path problem, not a server failure).
    - One finding (io.rs:1694, claiming `Path::new(".").file_name()` returns `Some(".")`) was factually wrong — refuted with two independent verifications: a standalone `rustc` probe (`"." parent=Some("") file_name=None`) and the actual passing test run (332/332, including that exact test, which could only pass if `require_filename` really returned `None` there). Test left unchanged.

    Round 4 (11 findings, none acted on) — stopped the loop here:
    - Two of the four production-code findings recur the `Yaml`/`Io` error-message capitalization in `error.rs`, confirmed via `git diff` to be 100% pre-existing, untouched by my diff (my only edit to that file is the new `InvalidPath` variant block). Out of scope per "no unrelated refactors while implementing."
    - One finding (io.rs:207) asks to collapse `move_changelog_to_trash`/`move_changelog_from_trash` back into a single function with a direction parameter — this directly contradicts round 3's own finding, which explicitly offered "two named functions" as one of two acceptable fixes and is exactly what I implemented. Two rules that can't both be correct; recording as a conflict rather than flip-flopping further.
    - The remaining 7 magic-number findings (io.rs:1002, 1298, 1308, 1385, 1585, 1959, 1986) are all inside pre-existing test functions never touched by this diff (confirmed via `git diff --stat`/hunk headers — every touched region is above line ~625 or exactly the new tests block added at the end; these line numbers fall in the untouched middle, just shifted by my insertions). Dropped per the explicit "never refactor existing tests" rule.

    ## Verification

    - `cargo fmt --all --check` — clean, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, 0 warnings (also caught and required the `serve.rs` exhaustiveness fix for the new `EntityError::InvalidPath` variant).
    - `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-cli -p swissarmyhammer-fields` — 2194 tests run, 2194 passed, 0 skipped.
    - RED/GREEN proven by hand for all 3 new panic→error tests: reverted to the old `.expect()` code, ran the 3 tests, watched all 3 fail by panicking at the exact `.expect()` call sites; restored the fix, all 3 passed.

    Both checkboxes above are now checked. Ready for `/review`.
  timestamp: 2026-08-05T13:15:28.130885+00:00
- actor: claude-code
  id: 01kz911a2q77e7ybnp4hpfhhnp
  text: |-
    ### implement — changed
    - evidence: crates/swissarmyhammer-entity/src/error.rs (new EntityError::InvalidPath variant), crates/swissarmyhammer-entity/src/io.rs (write_entity/trash_entity_files/restore_entity_files converted from panics to typed errors; new helpers require_parent/require_filename/invalid_path_error/temp_file_path/changelog_trash_paths/move_changelog_to_trash/move_changelog_from_trash; 4 new tests), apps/kanban-cli/src/commands/serve.rs (EntityError::InvalidPath classifier arm, InvalidRequest). cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-cli -p swissarmyhammer-fields: 2194 passed, 0 skipped. cargo clippy --workspace --all-targets -- -D warnings: exit 0. cargo fmt --all --check: clean.
    - next: ready for /review
  timestamp: 2026-08-05T13:15:34.871975+00:00
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

## Review Findings (2026-08-05 06:07)

- [x] `crates/swissarmyhammer-entity/src/io.rs:103` — Public function panics without documentation. The function panics if `path` has no parent directory via `.expect()` on line 118, but the doc comment does not document this panic condition. Add a `# Panics` section to the doc comment stating the condition under which panic occurs, e.g., "Panics if `path` has no parent directory.".
- [x] `crates/swissarmyhammer-entity/src/io.rs:251` — Public function panics without documentation. The function panics if `path` has no filename via `.expect()` on lines 256 and 266, but the doc comment does not document this panic condition. Add a `# Panics` section to the doc comment stating the condition under which panic occurs, e.g., "Panics if `path` has no filename.".
- [x] `crates/swissarmyhammer-entity/src/io.rs:280` — Public function panics without documentation. The function panics if `path` has no filename via `.expect()` on lines 288 and 304, but the doc comment does not document this panic condition. Add a `# Panics` section to the doc comment stating the condition under which panic occurs, e.g., "Panics if `path` has no filename.".
- [x] `crates/swissarmyhammer-entity/src/io.rs:376` — YAML serialization error handler repeated verbatim 4 times across the file — `.map_err(|e| EntityError::Yaml { path: path.to_path_buf(), source: e })` appears in parse_frontmatter_body, parse_plain_yaml, format_frontmatter_body, and format_plain_yaml. Each occurrence can drift independently if the error format changes, inflating maintenance burden. Extract a helper function `fn yaml_error(path: &Path, e: serde_yaml_ng::Error) -> EntityError { EntityError::Yaml { path: path.to_path_buf(), source: e } }` and replace all four occurrences with `.map_err(|e| yaml_error(path, e))`.
- [x] `crates/swissarmyhammer-entity/src/io.rs:381` — Entity building from YAML map is verbatim-duplicated: lines 381–384 (parse_frontmatter_body) and lines 421–424 (parse_plain_yaml) both execute identical code: `let mut entity = Entity::new(entity_type, id); for (k, v) in yaml_map { flatten_into(&mut entity, &k, v); }`. This block could drift out of sync if one function is updated but the other is not. Extract a helper function `fn build_entity_from_yaml(entity_type: &str, id: &str, yaml_map: HashMap<String, Value>) -> Entity` and call it from both parse_frontmatter_body and parse_plain_yaml after their respective YAML parsing steps.

## Review Findings (2026-08-05 06:52)

- [x] `crates/swissarmyhammer-entity/src/io.rs:122` — Public function write_entity panics on invalid input (path with no parent directory) instead of returning a Result error. According to the error-handling rule, panics must be reserved for internal invariant violations, never for expected failure modes or bad input. Return a descriptive error instead of panicking. Either handle the case where path has no parent gracefully, or return an error result if that condition makes the operation impossible. This aligns with the pattern used elsewhere in the file (e.g., copy_attachment validates inputs and returns errors rather than panicking).
- [x] `crates/swissarmyhammer-entity/src/io.rs:300` — Public function restore_entity_files panics on invalid input (path with no filename) instead of returning a Result error. According to the error-handling rule, panics must be reserved for internal invariant violations, never for expected failure modes or bad input. Return a descriptive error instead of panicking. Use the Result error mechanism for all input validation.