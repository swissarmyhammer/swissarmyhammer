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

The `duplication-parsed` tool rule decides the near-duplicate case, and it
supersedes `duplication`, `rust` and `swift` for every language the grammar
roster parses. The `duplicates` probe supplies the machine facts for the rest —
the languages the roster does not parse.

### The comparison is our own

The rule walks the named definitions the tree-sitter parse reports — every
function, every method, every type — normalizes each one, and pairs two
definitions by the length of the longest subsequence their normalized streams
share. A function normalizes its body positionally, so a renamed variable and a
substituted constant both fall out; a type normalizes its member types and
drops the member names.

Reading definitions rather than a window is what makes the exemptions possible:
the same parse that finds a copy decides which definitions are test code.

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
inside files that also hold production code. No path glob can reach those. The
rule this set ships compares tree-sitter definitions instead, so the exclusion
is the parse rather than the path.

### What the probe still answers

The tool rule reads only the languages the grammar roster parses. For every
other language the `duplication` prompt rule keeps running, and the
`duplicates` probe is the machine fact it reads. Cosine similarity is what the
probe measures; the tool rule needs none, because a renamed identifier falls
out of its normalization before the two definitions are compared.
