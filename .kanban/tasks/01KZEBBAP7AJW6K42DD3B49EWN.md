---
assignees:
- claude-code
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