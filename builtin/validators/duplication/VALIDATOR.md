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

The `duplication-parsed` tool rule decides the verbatim case, and it supersedes
`duplication`, `rust` and `swift` for every language the grammar roster parses.
The `duplicates` probe supplies the machine facts for the rest — the languages
the roster does not parse, and the near-verbatim copy no token matcher can see.

### `cpd-core` — the jscpd Rust engine, embedded

`cpd-core` 0.1.7 (MIT) is the core crate of the jscpd Rust rewrite. Its
`detect_prepared` entry point is the Rabin-Karp rolling-hash detector, and it
takes the token stream its caller supplies rather than tokenizing on its own.
That signature is what makes the rule possible: the tokens come from this
workspace's tree-sitter roster, so the same parse that finds a clone also
decides which blocks are test code.

The engine is embedded as a library, never run as a command. Its sibling
`cpd-tokenizer` is deliberately unused; it would pull the whole `oxc` parser
chain and give up the parse the exemptions depend on.

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
rule this set ships runs the engine's detector over a tree-sitter token stream
instead, so the exclusion is the parse rather than the path.

### What the probe still answers

Cosine similarity catches the near-duplicate that has renamed identifiers, and
a token matcher reads that pair as two different blocks. That shape is the
prompt rule's, and the `duplicates` probe is what it reads.
