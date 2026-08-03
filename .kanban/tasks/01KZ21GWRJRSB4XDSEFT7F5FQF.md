---
assignees:
- claude-code
position_column: todo
position_ordinal: e880
title: duplicates probe repeats its ~1.43 MB <changed-set> evidence once per file in the prompt
---
Found while diagnosing ^6jsxjbc (review batch budget exceeds the agent prompt cap).

# What was found

^6jsxjbc fixed the SYMPTOM — the batch budget and the agent's prompt cap now
agree, so an over-budget batch is caught and reported instead of failing as a
bare `invalid_params`. It did not fix the WASTE that made a batch this large in
the first place.

The real production log (`review sha 0c8b969b8~1..0c8b969b8`, `validator=duplication`)
showed a 14.9 MB prompt against a 5 MB cap. Rendered source was only ~0.1 MB of
that — about 1%. The dominant cost:

Each file block in that batch carries TWO probe results:
- a per-file `duplicates` result (9-123 KB, reasonable)
- the SAME shared `duplicates` result computed over `<changed-set>` (~1.43 MB),
  repeated IDENTICALLY on every file block in the batch

10 files x ~1.43 MB of duplicated evidence = ~14.3 MB sent for no reason. The
`<changed-set>` evidence is batch-scoped, not file-scoped — it does not change
per file — so repeating it per file multiplies its cost by the file count for
zero additional information.

# Why this still matters after ^6jsxjbc

^6jsxjbc raised the cap and made overflow visible instead of silent, which is
correct and necessary. But it did not stop the waste: a large commit still sends
the same multi-megabyte block N times, it just now either fits under the new cap
or reports as a named skip instead of a silent failure. Removing the duplication
would let more real commits complete in one pass instead of splitting into
skipped-file gaps, and it cuts real token cost on every `duplication`-validator
run, not just the ones near the cap.

# Investigate

- Where `<changed-set>` evidence is attached per file block vs. computed once
  per batch — find the assembly point in the fleet/scope code that builds each
  file's probe results.
- Whether the shared evidence can be emitted ONCE per prompt (e.g. before the
  per-file blocks, or as a single shared section) instead of once per file.

# Acceptance

- A test that packs N files needing the `duplicates` probe and asserts the
  shared `<changed-set>` evidence appears in the assembled prompt ONCE, not N
  times.
- The `duplication` validator's findings are unchanged — this is a payload-size
  fix, not a behavior change to what gets flagged.
- Re-measure the 71-file commit `0c8b969b8~1..0c8b969b8`: the `duplication`
  validator's prompt size should drop by roughly (N-1) x 1.43 MB for its
  largest batch.
- `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` passes.

#review #bug