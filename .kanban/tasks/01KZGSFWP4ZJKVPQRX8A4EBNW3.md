---
assignees:
- claude-code
depends_on:
- 01KZFHBM7MWFYAMK3SQADF0D7H
position_column: todo
position_ordinal: ffa880
title: 'duplication goes objective: sah duplicates tool rule supersedes the prompt rule'
---
## STANDING ORDER

This card is a decision, not a proposal. Your job is to implement it exactly.
- Do NOT re-run the jscpd-versus-probe evaluation. It happened (^3b49ewn) and its conclusion is overridden here.
- Do NOT keep the prompt rules running for matched files. Supersede them as stated.
- Do NOT soften the gate into evidence for an LLM. The detector decides. Zero LLM calls for this set on matched files is the acceptance bar.
- Do NOT file a follow-up card in place of doing the work.
- Stop and escalate ONLY when a step is impossible. Report the exact command and its output.

## The work

Correction to ^3b49ewn. That card asked the wrong question — it compared detectors and kept the existing implementation, which left duplication on the slowest component, an LLM pass. The point is review speed. Make the detector the decider: a deterministic tool rule, zero LLM.

Detector choice — decide with a look at the source, in this order:

1. jscpd ships a Rust engine: https://github.com/kucherenko/jscpd/tree/master/rust (MIT). If it is usable as a crate, embed it in the sah op directly — token-based Rabin-Karp duplicate detection, no Node install, language-aware tokenizers. Token-hash detection is the right algorithm for a deterministic near-verbatim gate: exact spans, exact repeat counts, no similarity fuzz.
2. If that crate is immature, implement the same algorithm natively: Rabin-Karp over the tree-sitter token stream we already produce, minimum window ~50 tokens. It is a small, well-known algorithm.
3. The existing cosine `duplicates` probe does not feed this rule — its strength (fuzzy near-matches) is what the deleted judgment tier consumed. ^adf0d7h (intra-file blindness) gets fixed if the cosine probe remains in use elsewhere; otherwise fold its test cases into this op's tests and close it against this card.

The rule:

- New sah op: run the detector over the file arguments, emit one finding per clone pair: `path:line: verbatim duplicate of <path:line> (<n> lines / <t> tokens)`. Deterministic gate only — a token-identical window over the minimum size IS a finding.
- Structural test exclusion, deterministic: drop a finding whose span sits inside a test node — `#[cfg(test)]` / `#[test]` in Rust, framework markers at the definition in other grammars — decided by the tree-sitter parse, never by file path.
- Inline suppression: a marker comment on the block (one form, e.g. `// sah:allow duplication <reason>`), honored across comment syntaxes. Exemptions live in code, never prose.
- Tool rule `duplication-parsed` in the `duplication` set: files scope, `run: sah <op> "$@"`, `supersedes: [duplication, rust, swift]`. Match lists the grammar roster's extensions; languages without a grammar keep the prompt path. Doctor names the sah binary; no install commands.
- Fixtures: fail = two identical 15-line blocks in one file (proves the intra-file case); pass = a suppressed copy with the marker, a duplicate pair inside `#[cfg(test)]`, and a below-minimum window. Extend the shipped-rules acceptance test. Acceptance: a review whose only defect is a pasted block reports it with zero LLM calls for the duplication set.

Depends on ^adf0d7h (or close it into this card per point 3).

#tool-validators #objectivity