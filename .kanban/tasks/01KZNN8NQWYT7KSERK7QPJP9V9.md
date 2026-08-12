---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzpnq7a8cfqjjz0y7n5qb456
  text: |-
    Research, before the change.

    The file changed after the card was written. Card ^xpf86ds put the two lookup tables into one `LANGUAGE_SPECS` roster of 16 rows, and made `is_identifier_kind` read `IDENTIFIER_KINDS` and `IDENTIFIER_KIND_ENDINGS`. `write_shape` is unchanged and is still the one place that does not use `is_attribute_kind`.

    Two facts the card does not hold:

    1. Python does NOT leak. `tree-sitter-python` puts a `decorator` inside a `decorated_definition`, beside the `class_definition`, not inside it. A probe of two decorated dataclasses that share every decorator gives 0 findings before the change. Python escapes the same way Rust escapes.

    2. `Normalization::Nameless` drops the `name` field of EVERY node under the walk, not only of the type declaration. An annotation node carries a `name` field, so the annotation NAME already drops out today. What reaches the shape is the punctuation and the arguments: `@`, `(`, `)`, `=`, each argument key, and a marker for each literal. Two classes under the same annotation block are therefore alike whatever the annotations are named.

    The languages that leak are the ones whose grammar nests the marker inside the definition node: Java (`modifiers`), C# (`attribute_list`), Swift (`modifiers`), PHP (`attribute_list`).
  timestamp: 2026-08-10T20:27:12.072876+00:00
- actor: claude-code
  id: 01kzpnqkk52b4rns0w842egy7b
  text: |-
    The change, and what it measures.

    `write_shape` now skips every kind in `ATTRIBUTE_KINDS`, exactly as the card writes it. Four tests were written first and all four failed first, each for the correct reason:

    - `a_java_annotation_contributes_no_token_to_the_shape` — the shape held `@` and `#str`.
    - `a_csharp_attribute_list_contributes_no_token_to_the_shape` — the shape held `[` and `#str`.
    - `two_classes_that_share_only_their_annotations_are_not_reported` — the pair WAS reported: `class Band` against `class Row`, 50 tokens, 98% alike.
    - `two_classes_that_repeat_their_members_are_reported_though_one_is_annotated` — the pair scored 98, not 100, because one class carries `@Entity` and the other does not.

    All four pass after the change. All 1119 tests of `swissarmyhammer-sem` and `swissarmyhammer-code-context` pass. `cargo fmt` is clean and `cargo clippy --workspace --all-targets -- -D warnings` is clean.

    Counts, measured with a release `sah` built before the change and a release `sah` built after it, over the same working tree at HEAD 87a8c3da7:

    | Probe | Before | After |
    |---|---|---|
    | Java, two classes under one annotation block | 1 | 0 |
    | C#, two classes under one attribute list | 1 | 0 |
    | Swift, two classes under one modifier list | 1 | 0 |
    | PHP, two classes under one attribute list | 1 | 0 |
    | Python, two dataclasses under one decorator block | 0 | 0 |
    | Rust, all 1191 tracked `.rs` files of this workspace | 403 | 386 |

    The Rust baseline is 403, not the 416 the card states and not the 406 the dispatch states. The card's number came from 1183 files; the tree now holds 1191.

    The probe files are in the session scratchpad, not in the repository.
  timestamp: 2026-08-10T20:27:24.645827+00:00
- actor: claude-code
  id: 01kzpnr5r4r27dgsncktvs1jzd
  text: |-
    BLOCKER. Two of the four "Done when" items cannot both hold.

    Item 1 says `write_shape` skips every kind in `ATTRIBUTE_KINDS`. Item 3 says the Rust count over this workspace does not move. The measurement says the Rust count moves from 403 to 386.

    The cause is a grammar fact the card does not hold. The card says Rust escapes because `tree-sitter-rust` makes `attribute_item` a SIBLING of `struct_item`. That is correct for the attribute ON the item. It is not correct for an attribute INSIDE the item. A field attribute, a variant attribute and an attribute on a statement in a function body are all children of the definition node, so `write_shape` reaches them today.

    The 403 findings become 386: 20 go away and 3 appear. Each one is the case the card describes.

    Findings that go away, by cause:

    - A `thiserror` enum. `enum EmbeddingError` in `crates/llama-embedding/src/error.rs` measures 115 tokens before and 44 after, so 71 of its 115 tokens were `#[error("...")]` attributes on its variants. It still matches its partner in `crates/ane-embedding/src/error.rs`, now at 95% rather than 94%.
    - A `clap` enum. `enum Commands` in `apps/kanban-cli/src/cli.rs` matched `enum Commands` in `apps/code-context-cli/src/cli.rs` at 99% over 81 tokens. The tokens were `#[command(...)]` and `#[arg(...)]`.
    - A `serde` record. `struct McpFrontmatter`, `struct MoveTask`, `struct PlanEntry` and five more matched on their `#[serde(...)]` attributes.
    - A `#[cfg(target_os = "macos")]` match arm. `load`, `embedding_dimension`, `is_loaded` and `embed_single` in `crates/swissarmyhammer-embedding/src/embedder.rs` each measure 44 to 50 tokens before, and the attribute is what carried them over the 40-token minimum. Under the minimum they are no longer candidates.

    Findings that appear: three pairs that the attributes had held apart, among them `struct WebSearchRequest` against `struct SearchUrl` at 97%.

    Every one of the 23 is the rule reading an attribute as code. No other behaviour changed.

    There is no narrower fix inside the card. The card writes the exact code and says "skips every kind in `ATTRIBUTE_KINDS`", and a comment is skipped everywhere in the walk, so an attribute is too. To hold the Rust count at 403 the walk would have to keep an attribute inside a definition, which contradicts item 1.

    A person must decide which item stands. The implementation is in the working tree, green, and ready either way.

    Two more facts a decision needs:

    1. `modifiers` is in `ATTRIBUTE_KINDS`. In Java and in Swift that node also carries `public`, `static`, `final` and `open`. Skipping it drops those words from the shape as well as the annotations.
    2. The rule body `builtin/validators/duplication/rules/duplication-parsed.md` states 416 findings over 1183 files. It was already wrong before this change: the tree holds 1191 files and reports 403. If this change stands, the whole calibration table of that rule needs a new measurement.
  timestamp: 2026-08-10T20:27:43.236645+00:00
- actor: claude-code
  id: 01kzpnsv4dn5jbjdegxpm50z3s
  text: |-
    ### implement — stuck
    - evidence: 2 files — crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs, crates/swissarmyhammer-code-context/src/ops/find_duplication.rs. Probe counts before/after: Java 1/0, C# 1/0, Swift 1/0, PHP 1/0, Python 0/0, Rust over 1191 tracked `.rs` files 403/386. Commands: `cargo nextest run --workspace` (14038 tests, 14034 pass, the 4 known pre-existing failures of ^bh5ncd0 and no others), `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` clean. The three `review_e2e` failures carry the same text before and after the change, measured by taking the change out and putting it back.
    - next: a person decides between "Done when" item 1 and item 3. They cannot both hold. New card ^kqhmzbe holds the stale rule-body measurement.
  timestamp: 2026-08-10T20:28:37.901396+00:00
- actor: claude-code
  id: 01kzpny8qwxqncc6cgmq3ntx99
  text: |
    ### finish iteration 1 — stuck
    - implement: stuck — the fix is written, green and correct, but "Done when" item 1 and item 3 cannot both hold. A person must decide.
    - 4 tests were written first and all 4 failed first, each for the right reason. The pair was reported at 50 tokens and 98 percent alike before the change.
    - measured, with a release `sah` built before the change and after it on the same tree:
      - Java, two classes under one annotation block: 1 -> 0
      - C#, two classes under one attribute list: 1 -> 0
      - Swift, two classes under one modifier list: 1 -> 0
      - PHP, two classes under one attribute list: 1 -> 0
      - Python, two dataclasses under one decorator block: 0 -> 0
      - Rust, 1191 tracked `.rs` files: **403 -> 386**
    - THE CONFLICT: the card says the Rust count must not move, because "tree-sitter-rust makes `attribute_item` a sibling of `struct_item`". That is true for an attribute ON an item and false for an attribute INSIDE one. `#[serde(...)]` on a field, `#[error("...")]` on a variant, `#[command(...)]` on a clap variant and `#[cfg(target_os = "macos")]` on a match arm are all children of the definition node, so `write_shape` reads them today. 20 findings go and 3 appear, and all 23 are the rule reading an attribute as code. `enum EmbeddingError` in `crates/llama-embedding/src/error.rs` measures 115 tokens before and 44 after; 71 of its tokens were `#[error(...)]`.
    - there is no narrower fix inside the card. A comment is skipped everywhere in the walk, so an attribute is too.
    - two more facts a decision needs: `modifiers` is in `ATTRIBUTE_KINDS`, so Java and Swift also lose `public`, `static`, `final` and `open` from the shape; and `builtin/validators/duplication/rules/duplication-parsed.md` was already stale before this change (it states 416 over 1183 files, the tree reports 403 over 1191). That file was not touched. Card ^kqhmzbe is filed for the re-measurement.
    - test: `cargo nextest run --workspace` 14038 run, 14034 passed, 0 skipped; the 4 failures are the known ^bh5ncd0 set. The change was taken out, the three `review_e2e` tests were run again, and the change was put back: the failure text is identical. The fixture's duplicate `sum_amounts` carries no attribute, so the change cannot reach it.
    - commit: 8dcbc6b9a — a CHECKPOINT of the green work, not a closure. The card stays open.
    - next: a person decides whether the Rust count is permitted to move from 403 to 386. If yes, item 3 of "Done when" is wrong and must be rewritten before the card can pass its own gate.
  timestamp: 2026-08-10T20:31:02.908399+00:00
- actor: claude-code
  id: 01kzpremt9satmavfgshwggyqx
  text: |-
    ### review — clean
    - scope: `review sha HEAD~1..HEAD` (commit 8dcbc6b9a)
    - engine: 8 tasks attempted, 0 failed, 0 skipped, 0 skipped_files; 0 findings, 0 confirmed, 0 refuted
    - files: the commit touches 6 files. 2 are source (`crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs`, `crates/swissarmyhammer-code-context/src/ops/find_duplication.rs`). The other 4 are the kanban records of this card and of ^kqhmzbe.
    - item 1 holds: `write_shape` skips `is_attribute_kind(child.kind())`, and `is_attribute_kind` is `ATTRIBUTE_KINDS.contains(&kind)`. All 7 kinds are thus skipped.
    - item 2 holds: `two_classes_that_share_only_their_annotations_are_not_reported` passes, and `two_classes_that_repeat_their_members_are_reported_though_one_is_annotated` proves the gate stays closed. 26 of 26 `find_duplication` tests pass. 37 of 37 `duplication` tests pass.
    - item 3 holds, measured again: a release `sah` built from 87a8c3da7 reports 403 findings, and a release `sah` built from 8dcbc6b9a reports 386, both over the same 1191 tracked `.rs` files. The delta is 19 pairs removed, 2 pairs added and 1 pair measured again (`EmbeddingError`, 115 tokens to 44). Each one has an attribute cause: the removed `embedder.rs` functions matched on `#[cfg(target_os = "macos")]` in their match arms, and the added `WebSearchRequest`/`SearchUrl` pair holds the same 7 field types and differed only by `#[serde]` and `#[schemars]`.
    - item 4 holds: `a_rust_test_function_contributes_no_definition`, `a_rust_cfg_test_function_contributes_no_definition`, `a_java_method_is_read_and_the_annotated_test_is_not` and `a_csharp_method_is_read_and_the_fact_is_not` all pass. `marked_by_attribute` and `definition_range` are not changed by this commit.
    - note, not a finding: the commit subject of 8dcbc6b9a says "card is stuck". A person has since accepted the count drop, so that word is out of date. The code is correct.
    - note, not a finding: `builtin/validators/duplication/rules/duplication-parsed.md` still states 416 over 1183 files. Card ^kqhmzbe owns that re-measurement.
    - next: card moves to done.
  timestamp: 2026-08-10T21:14:56.713558+00:00
- actor: claude-code
  id: 01kzprfyr3yesnn26ngskbj0fn
  text: |
    ### finish iteration 2 — clean
    - the conflict of iteration 1 was resolved by a person on 2026-08-10: the Rust count drop is accepted, and acceptance item 3 is rewritten to state the measured result. The card's wrong premise about the Rust grammar is corrected in the description.
    - review: clean — 0 findings, 8 tasks attempted, 0 failed, 0 skipped, `skipped_files` empty. 2 of 2 source files reviewed of the 6 files the commit touches.
    - the reviewer re-measured the count itself rather than trusting the card: it built a release `sah` from 87a8c3da7 in a detached scratch worktree and one from 8dcbc6b9a in the main tree, then ran both over the same 1191 files. 403 and 386.
    - the delta reads as 19 pairs removed, 2 pairs added, 1 pair re-measured. Two were inspected at the source:
      - REMOVED: the `crates/swissarmyhammer-embedding/src/embedder.rs` function pairs matched on `#[cfg(target_os = "macos")]` on match arms. That is the in-item Rust leak the corrected description names.
      - ADDED: `WebSearchRequest` against `SearchUrl` carry the same 7 field types and differed only by `#[serde]` and `#[schemars]`. The fix uncovered a real duplicate the attributes had masked. A finding that APPEARS is as much a proof of the fix as one that goes.
    - markers still work, proved by test: `a_rust_test_function_contributes_no_definition`, `a_rust_cfg_test_function_contributes_no_definition`, `a_java_method_is_read_and_the_annotated_test_is_not`, `a_csharp_method_is_read_and_the_fact_is_not`. `marked_by_attribute` and `definition_range` are untouched.
    - commit: 8dcbc6b9a. Its subject still says "card is stuck", which the person's decision has made out of date. The commit is not amended, because it is already the parent of the review that passed.
    - open, owned by ^kqhmzbe: `builtin/validators/duplication/rules/duplication-parsed.md` states 416 over 1183 files. The tree now reports 386 over 1191.
  timestamp: 2026-08-10T21:15:39.651015+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffda80
title: duplication must never read an attribute, an annotation or a decorator as code
---
An attribute is a declaration to the compiler, not code a reader can make dry. `#[derive(Debug, Clone)]`, `@Override`, `@dataclass`, `[Fact]` and `@Test` repeat because the language makes them repeat. A finding on them asks the author to change something they cannot change, so it is not a requirement. See [[dont-fix-wrong-rule]].

`duplication-parsed` reads them today.

## Where

`crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs`.

`write_shape` builds the token stream a definition compares by. It skips a comment:

```rust
if is_comment_kind(child.kind()) || child.start_byte() >= child.end_byte() {
    continue;
}
```

It has no such test for an attribute. The file already holds `is_attribute_kind` and `ATTRIBUTE_KINDS` (`attribute_item`, `attribute_list`, `attribute`, `annotation`, `marker_annotation`, `modifiers`, `decorator`), and `write_shape` is the one place that does not use them.

`shape_of` decides what the walk starts from:

- `Normalization::Positional`, used for a function or a method, starts at `definition_body(node)`.
- `Normalization::Nameless`, used for a type — `record(...)` — starts at the definition node itself. Every child is walked, the annotation children included.

The leak is on type declarations, in the languages whose grammar nests the marker inside the definition node: the Java `modifiers` child, the C# `attribute_list` child, the Python `decorator`, and the Swift and PHP attributes.

**Rust leaks too.** An earlier version of this card said Rust escapes, because tree-sitter-rust makes `attribute_item` a **sibling** of `struct_item`. That is correct for the attribute ON an item and wrong for an attribute INSIDE one. A `#[serde(...)]` on a field, a `#[error("...")]` on a variant, a `#[command(...)]` on a clap variant and a `#[cfg(target_os = "macos")]` on a match arm are all children of the definition node, so `write_shape` reads all of them today. `enum EmbeddingError` in `crates/llama-embedding/src/error.rs` measures 115 tokens before the fix and 44 after it: 71 of its tokens were `#[error(...)]` strings.

## The fix

Skip an attribute in `write_shape` exactly as a comment is skipped:

```rust
if is_comment_kind(child.kind())
    || is_attribute_kind(child.kind())
    || child.start_byte() >= child.end_byte()
{
    continue;
}
```

This makes the rule read what it says it reads. It does not weaken the gate: two types that are the same after their annotations are removed are still the same.

## What does not change

An attribute stays readable as a **marker**. `marked_by_attribute` reads it to tell test code from other code, and `definition_range` reads it to put the range start at `#[test]`. Reading a marker is not the same as counting its tokens as duplicated code. ^xpf86ds corrects the marker reading and stands on its own.

JavaScript and TypeScript need nothing. They have no attribute. A test there is a call — `test`, `describe`, `it` — which `JAVASCRIPT_TEST_SPEC.calls` already holds.

## Measured

Measured with a release `sah` built before the change and after it, on the same tree:

| Probe | before | after |
|---|---|---|
| Java, two classes under one annotation block | 1 | 0 |
| C#, two classes under one attribute list | 1 | 0 |
| Swift, two classes under one modifier list | 1 | 0 |
| PHP, two classes under one attribute list | 1 | 0 |
| Python, two dataclasses under one decorator block | 0 | 0 |
| Rust, 1191 tracked `.rs` files | **403** | **386** |

20 Rust findings go away and 3 appear. Every one of the 23 is the rule reading an attribute as code.

`modifiers` is in `ATTRIBUTE_KINDS`, so Java and Swift also lose `public`, `static`, `final` and `open` from the compared shape. That is wider than the word "annotation", and it is deliberate: a modifier is a declaration to the compiler by the same argument.

## Done when

- [x] `write_shape` skips every kind in `ATTRIBUTE_KINDS`
- [x] A probe proves a pair that matched only on annotations no longer matches
- [x] The Rust count over this workspace falls from 403 to 386, and each removed finding is the rule reading an attribute as code
- [x] `#[test]`, `#[cfg(test)]`, `@Test` and `[Fact]` still mark test code

## Resolved

The first pass reported this card stuck: the acceptance list required the Rust count to stay at 403, and the fix moves it to 386. A person decided on 2026-08-10 that the drop is correct, because the removed findings are exactly the defect this card names. The wrong premise about the Rust grammar is corrected above, and acceptance item 3 now states the measured result.

`builtin/validators/duplication/rules/duplication-parsed.md` holds a stale measurement, and held one before this card started: it states 416 over 1183 files. Card ^kqhmzbe owns the re-measurement.

#tool-validators #objectivity