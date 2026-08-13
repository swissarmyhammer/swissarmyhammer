---
name: duplication
description: >-
  Flag verbatim or near-verbatim copied blocks. Machine-written code trends
  toward copy-paste; copies drift out of sync and inflate the surface area.
  Two blocks that differ only by a value are one function with an argument —
  extract a shared function and parameterize the difference.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
probes:
  - duplicates
---

# Duplication Validator

Promote one of the exact problems machine-written code keeps reintroducing —
duplicated, copy-pasted code — into a first-class, focused review concern. This
validator does one thing: catch verbatim and near-verbatim copied blocks so they
become a shared function instead of N copies a human must keep in lockstep.


** IMPORTANT ** This rule does not apply to test code.

## Which tools this set uses

No tool decides this set. The `duplication`, `rust` and `swift` prompt rules
decide, and the `duplicates` probe supplies the machine facts they read.

### `cpd-core` — embedded, then removed

`cpd-core` 0.1.7 (MIT), the core crate of the jscpd Rust rewrite, decided the
first version of this rule. Its `detect_prepared` entry point is a Rabin-Karp
rolling-hash detector over a token stream, which reports where a run of N
tokens is spelled twice.

That is the wrong question. A run is not a definition: it pairs the tail of one
function with the head of another, and it reports boilerplate spanning two
definitions. It also never sees a whole definition, so it cannot say how alike
two definitions are. A sequence comparison answers that and a rolling hash
cannot, so the dependency came out with the window. Its sibling
`cpd-tokenizer` was never used.

### `jscpd` as a command — rejected, and why that verdict stands

A token clone detector for about 150 languages. Version 5.0.14, run over all
1155 tracked `.rs` files in this repository, against the same files the
`duplicates` probe indexed.

Most of what it adds is test code. 60.6% of its Rust clone instances sit in
inline `#[cfg(test)]` modules, and a further 17.0% sit under `tests/`. Of its
3199 clusters, 2428 are all-test. This rule does not apply to test code, so
those findings are noise.

A tool rule that ran `jscpd` as a command could not remove them. It scopes its
input only by path glob, and 4857 clone instances sit in inline test modules
inside files that also hold production code. No path glob can reach those. A
reader can, because the `#[cfg(test)]` module a block sits in stands in the
same file the reader has open.

### What the probe answers

The `duplicates` probe is the machine fact the prompt rules read. Cosine
similarity is what it measures, and it names the verbatim and near-verbatim
blocks it matched, both against the existing index and across the changed set.
