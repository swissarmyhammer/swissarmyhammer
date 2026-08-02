---
assignees:
- claude-code
position_column: todo
position_ordinal: d680
title: complexity validator is nondeterministic on tag_parser.rs — same file, different findings per run
---
The `complexity` validator returns a different finding set on repeated runs over the same unchanged file. Observed on `crates/swissarmyhammer-kanban/src/tag_parser.rs` on 2026-07-31 while working ^tnr56gg.

## What was seen

Across repeated runs on one unchanged file:

- Some runs flagged `collect_line_tags` and `edit_line_markers` with "match arms contain code at depth 4".
- Other runs did not flag them at all.
- Later runs additionally raised stylistic items (a missing module `# Examples` section, a `b'` backtick literal in three functions) that earlier runs had not raised.

The match-arm findings were **false positives** against the validator's own documented rule. `builtin/validators/complexity/rules/cognitive-complexity.md` counts nested *conditions*; both functions were two-arm `Option` matches sitting at depth 2. They were flattened from `match` to `if let`/`else` anyway, purely to remove the ambiguity — not because the rule required it.

## Why it matters

The review gate is treated as binary: any open finding means a task is not done. That contract only holds if the gate is deterministic. Nondeterminism produces three concrete failures:

1. **A task can pass or fail on a coin flip.** The same commit reviewed twice can yield clean or not-clean.
2. **It manufactures busywork.** Work gets done to satisfy a finding that a re-run would not have raised, as happened here with the two match flattenings.
3. **It corrupts the finish loop's stuck-detection guardrail.** That guardrail declares a task stuck when the *same* finding survives 3 iterations. A validator whose finding set churns can hide a genuinely persistent problem behind a rotating cast of findings, or trip the guardrail on findings that were never really the same one.

## Investigate

- Whether the validator prompt or its file batching is order-dependent or size-dependent (`batch_size` inlines file bytes per review batch, so a file near a boundary may be split differently between runs).
- Whether the depth rule is being applied by agent judgment where the documented rule is stricter than the prompt conveys — `match` arms counting as a nesting level contradicts the cognitive-complexity doc.
- Whether sampling or temperature in the validator agent is the source, and whether it should be pinned for validators.

## Acceptance

- The same file reviewed N times with no change in between yields the same finding set. Demonstrate with a repeated-run harness, not a single run.
- If `match` arms are intended to count toward nesting depth, `cognitive-complexity.md` says so explicitly. If they are not, the validator stops raising them.

Do not "fix" this by weakening the rule to make findings disappear. The goal is a stable gate, not a quiet one.

Found while driving ^tnr56gg through the finish loop. Related: ^fpcbeth (frontmatter split defect found in the same run). #bug #review