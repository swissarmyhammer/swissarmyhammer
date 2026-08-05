---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8c80
title: 'review engine: escalate "a file no validator could read" beyond a warning'
---
Design question recorded from ^t1y1c37, not built there.

# Problem

When one (validator, file) pair's rendered prompt exceeds the agent's prompt cap, the engine prints one warning line and reports the rest of the review as normal. Nothing fails. A file that stays over the cap is permanently outside that validator's coverage, and every review of it reads "clean" for that dimension. `crates/mirdan/src/install.rs` sat in that state until the split (^t1y1c37): 567352 rendered bytes against the 476042-byte budget, skipped by `duplication` on every run.

# Proposal to evaluate

Treat "a file no validator could read" as a coverage failure, not a warning:

- The `ReviewReport` carries the skipped pairs as structured data (`counts.skipped` exists; add the file list), so orchestrators can gate on it.
- The `/review` skill and the finish loop treat a skipped pair as a finding on the task: "file X exceeds the review prompt cap — split it", so the gate fails until the file shrinks.
- Optionally: the engine emits the skip as a CONFIRMED finding itself, so no consumer needs special handling.

# Acceptance

- A review whose scope contains an over-cap file cannot end `clean`; the skip is visible as a finding or a non-zero gate, not only as a warning line in markdown.
- A test proves the behavior with a synthetic over-cap file. #review #design