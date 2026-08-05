---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8580
title: 'TreeSitterProbe trait: file + parse + logic'
---
Add a second probe family beside the `ProbeOp` catalog in `swissarmyhammer-validators/src/review/probes.rs`.

A tree-sitter probe is a trait implementation. Input: one changed file and its tree-sitter parse. Output: `ProbeRow`s.

Work:
- Define the trait: name, `ProbeKind`, and `run(file, parse, diff_context) -> Vec<ProbeRow>`.
- Diff-aware probes need the before AND after parse of a changed file. Put both in the probe context.
- Parse each file one time per review. Share the tree across every probe that runs on that file.
- Register trait probes in the existing probe catalog. Validators declare them by name in `probes:` exactly as today. `probe_exists` and `check validators` see them.
- Reuse the tree-sitter grammars and language routing that the code-context chunker already owns. Do not add a second grammar roster.
- Probe results flow to rules on the existing `ProbeResult` path. No new plumbing per probe.

Acceptance:
- A trivial trait probe (for example: function count per file) registers, runs on a review, and its rows reach the validator prompt.
- One parse per file per review, proven by test.
- A file whose language has no grammar produces one "could not compute" row, same as the complexity probe does today.

#tool-validators