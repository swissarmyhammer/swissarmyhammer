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

## Which tools this set uses, and which it rejects

The `duplicates` probe supplies the machine facts. No tool rule supersedes this
validator.

### `jscpd` — rejected

A token clone detector for about 150 languages. Version 5.0.14, run over all
1155 tracked `.rs` files in this repository, against the same files the
`duplicates` probe indexed.

It adds almost no new information. The probe already knows 7102 of jscpd's 7207
Rust clone sites (98.5%). The probe knows every site of 3147 of jscpd's 3199
clusters (98.4%). The unique yield of jscpd is 60 pairs (1.5%), median 8 lines.
Of those 60 pairs, 35 are file-header `use` blocks, which the symbol chunker
does not emit.

The probe finds 3.5 times more sites than jscpd. Cosine similarity catches the
near-duplicate that has renamed identifiers. A token matcher reads that pair as
two different blocks. The renamed-identifier copy is the shape this rule
targets.

Most of what jscpd adds is test code. 60.6% of its Rust clone instances sit in
inline `#[cfg(test)]` modules, and a further 17.0% sit under `tests/`. Of its
3199 clusters, 2428 are all-test. This rule does not apply to test code, so
those findings are noise.

A tool rule with `supersedes: duplication` is therefore not possible. jscpd
scopes its input only by path glob, and it has no knowledge of `#[cfg(test)]`.
4857 clone instances sit in inline test modules inside files that also hold
production code. No path glob can remove them.
