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
- actor: claude-code
  id: 01kzfmt4p2xmd5fhgqwxqkkvv3
  text: |
    Recorded outcome (b) in `builtin/validators/duplication/VALIDATOR.md`. Documentation only. No probe, no tool rule, no code.

    The new section copies the heading style of `builtin/validators/code-hygiene/VALIDATOR.md`, which is the precedent for this record: a `## Which tools this set uses, and which it rejects` heading, then one `### <tool> — rejected` block for each measured candidate.

    The section holds every measured number from the evaluation comment:
    - overlap: 7102 of 7207 sites (98.5%), every site of 3147 of 3199 clusters (98.4%)
    - unique yield: 60 pairs (1.5%), median 8 lines, 35 of 60 are file-header `use` blocks
    - the probe finds 3.5 times more sites, because cosine similarity catches renamed-identifier near-duplicates
    - test noise: 60.6% inline `#[cfg(test)]`, 17.0% more under `tests/`, 2428 of 3199 clusters all-test
    - `supersedes: duplication` is not possible: path-glob-only scoping, 4857 instances in inline test modules inside production files

    `{"op": "dump validators", "paths": ["builtin/validators/duplication/VALIDATOR.md"]}` returns `rule_count: 0` for the `md` extension. No review rule applies to this file.

    `cargo nextest run -p swissarmyhammer-validators`: 523 tests run, 523 passed, 0 skipped. No test asserts on the contents of this file.
  timestamp: 2026-08-08T02:56:38.082887+00:00
- actor: claude-code
  id: 01kzfmt9me1dv4tmxce8hm8v1s
  text: |
    ### implement — changed
    - evidence: 1 file — builtin/validators/duplication/VALIDATOR.md (+32 lines). `cargo nextest run -p swissarmyhammer-validators`: 523 run, 523 passed, 0 skipped.
    - next: /review
  timestamp: 2026-08-08T02:56:43.150039+00:00
- actor: claude-code
  id: 01kzfnhj2pwz8bs4z6tp10hh6k
  text: |-
    ### test — green
    - evidence: `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean (0 warnings); `cargo nextest run --workspace` — 13782 tests run: 13782 passed (139 slow), 0 failed, 0 skipped.
    - next: ready for review/commit.
  timestamp: 2026-08-08T03:09:25.462337+00:00
- actor: claude-code
  id: 01kzfns6mynapye8nz9xfttmh8
  text: |
    ### review — clean
    - evidence: `{"op": "review sha", "sha": "HEAD~1..HEAD"}` (= b3ab023d8) returned "Nothing in scope to review" — counts: 0 findings, 0 confirmed, 0 refuted, 0 attempted, 0 failed, 0 skipped. The commit touches only `.kanban/**` and `builtin/validators/duplication/VALIDATOR.md`. All 14 registered validators match source-code extensions only; none has a glob for `.md`, so no validator/file pair was formed. This confirms the implement step's `dump validators` result of `rule_count: 0` for the `md` extension.
    - next: task moves to done. No findings section appended, because the engine returned none.
  timestamp: 2026-08-08T03:13:35.902487+00:00
- actor: claude-code
  id: 01kzfnvj787z0g6c58hzt8fbxm
  text: |
    ### finish iteration 1 — clean
    - implement: changed — 1 file, builtin/validators/duplication/VALIDATOR.md
    - test: green — cargo nextest run --workspace, 13782 passed, 0 failed, 0 skipped; fmt, clippy -D warnings clean
    - commit: b3ab023d8 docs(validators): record jscpd rejected against duplicates probe (^3b49ewn)
    - review: clean — 0 findings, 0 of 14 validators attempted
    - task moved to done by the review gate

    Note on the review: the pass was vacuous. No validator declares a `.md` glob, so no validator/file pair could form for a documentation-only commit. The `clean` result means "nothing in scope", not "the text was checked".
  timestamp: 2026-08-08T03:14:53.288740+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffc680
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