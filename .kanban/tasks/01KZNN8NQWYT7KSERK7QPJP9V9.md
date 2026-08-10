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
position_column: doing
position_ordinal: '8480'
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

- `Normalization::Positional`, used for a function or a method, starts at `definition_body(node)`. A body holds no attribute, so a callable escapes.
- `Normalization::Nameless`, used for a type — `record(...)` — starts at the definition node itself. Every child is walked, the annotation children included.

So the leak is on type declarations, in the languages whose grammar nests the marker inside the definition node: the Java `modifiers` child, the C# `attribute_list` child, the Python `decorator`, and the Swift and PHP attributes. Rust escapes by structure alone, not by intent — tree-sitter-rust makes `attribute_item` a **sibling** of `struct_item`, which is why `definition_range` and `attribute_texts` both walk `prev_sibling`. A grammar change would put the derives straight into the shape.

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

## Measure

Baseline over the 1183 tracked `.rs` files of this workspace is 416 findings, from ^xpf86ds. Rust has no leak by the reading above, so the Rust count must not move. Prove the change with a language that does leak:

- Write a probe with two Java classes, or two Python dataclasses, that share only their annotations and differ in every member. Confirm the pair is reported before the change and is not reported after it.
- Record the before and after count for each language you probe.

## Done when

- [x] `write_shape` skips every kind in `ATTRIBUTE_KINDS`
- [x] A probe proves a pair that matched only on annotations no longer matches
- [ ] The Rust count over this workspace is unchanged — MEASURED FALSE, see the blocker below
- [x] `#[test]`, `#[cfg(test)]`, `@Test` and `[Fact]` still mark test code #tool-validators #objectivity

## Blocker

Item 1 and item 3 cannot both hold. The fix the card writes moves the Rust count from 403 to 386 over the 1191 tracked `.rs` files this tree now holds.

The card says Rust escapes because tree-sitter-rust makes `attribute_item` a sibling of `struct_item`. That is correct for the attribute ON the item. It is not correct for an attribute INSIDE the item: a `#[serde(...)]` on a field, a `#[error("...")]` on a variant, a `#[command(...)]` on a `clap` variant and a `#[cfg(...)]` on a match arm are all children of the definition node, so `write_shape` reads all of them today.

20 findings go away and 3 appear. Every one of the 23 is the rule reading an attribute as code. The comment thread names each cause with an example.

To hold the Rust count at 403, the walk would have to keep an attribute that stands inside a definition. That contradicts item 1. A person must decide which item stands. The implementation is in the working tree, green, with four tests that each failed first.