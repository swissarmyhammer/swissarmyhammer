---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzfhbbnsn5wcfwn841w4any3
  text: |
    ### evaluation — evidence gathered, outcome (b) reject jscpd

    Measured on this repo. jscpd v5.0.14 and the `duplicates` probe both ran over all 1155 tracked `.rs` files.

    **jscpd**: 6441 clones repo-wide, 12.50% duplicated lines. Rust: 4008 clone pairs, 7207 sites, 3199 clusters.

    **probe** (`find_duplicates_in`, cosine >= 0.85 over tree-sitter chunks): 25066 sites, 91480 pairs, 214 clusters.

    **Overlap**:
    - The probe already knows 7102 of jscpd's 7207 Rust sites (98.5%).
    - The probe knows every site of 3147 of jscpd's 3199 clusters (98.4%).
    - True probe misses: 60 pairs (1.5%). Median 8 lines. 35 of the 60 are file-header `use` blocks that the symbol chunker does not emit.
    - The probe finds 3.5x more sites, because cosine similarity catches renamed-identifier near-duplicates that a token matcher treats as distinct. That is the "differ only by a renamed variable" shape the rule names.

    **Noise cost of adding jscpd**: 60.6% of its 8016 Rust clone instances sit in inline `#[cfg(test)]` modules, and 17.0% more sit under `tests/`. 2428 of its 3199 clusters are all-test. `duplication/VALIDATOR.md` states the rule does not apply to test code. jscpd also parsed fenced code blocks in `ideas/*.md` and `standards/mcp.md` as pseudo-files.

    **Third path is not available**: jscpd scopes only by path glob. It has no notion of `#[cfg(test)]`, and 4857 clone instances sit in inline test modules inside files that also hold production code. A path filter cannot exclude them. This is the `no-path-based-test-exclusion` rule verbatim. A tool rule with `supersedes: duplication` is therefore impossible, and it was not forced.

    **Outcome (b)**: reject jscpd. Record the reason in `builtin/validators/duplication/VALIDATOR.md`.

    **Separate gap, filed on its own card**: `find_duplicates_in` removes the source file from its own corpus, so the probe cannot report a duplicate inside one file. Measured 0 intra-file pairs out of 91480. jscpd found 470 prod-to-prod intra-file pairs. This is a probe fix, not an argument for jscpd.
  timestamp: 2026-08-08T01:56:07.993440+00:00
position_column: todo
position_ordinal: ff9480
title: 'duplication: evaluate jscpd against the duplicates probe'
---
Evaluate `jscpd` (one tool, ~150 languages) against the in-house `duplicates` tree-sitter probe that the duplication validator already consumes.

The blocker: the duplication rule exempts test code, and Rust holds tests inline in source files. A path filter cannot exclude them (see the no-path-based-test-exclusion rule). jscpd cannot judge what is a test.

Possible outcomes — pick one with evidence:
- jscpd output feeds the prompt rule as a second machine source, and the LLM keeps the test-code judgment. This is the probe + prompt tier, not a tool rule.
- The `duplicates` probe already covers the need. Reject jscpd. Record why in the duplication VALIDATOR.md.

A full tool rule with `supersedes: duplication` is possible only if the test-exemption question gets a deterministic answer. Do not force it.

Evidence to gather: run jscpd on this repository; compare its clusters with the probe's clusters; count the findings that land in inline test modules.

#tool-validators