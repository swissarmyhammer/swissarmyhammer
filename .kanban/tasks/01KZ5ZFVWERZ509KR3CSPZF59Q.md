---
assignees:
- claude-code
position_column: todo
position_ordinal: f880
title: 'probes.rs: render_probe_evidence exceeds complexity gate; run_probes takes &[String] instead of &[&str]'
---
Found by `mcp__sah__review` while working task ^401xdvp (unrelated to that task's diff — flagged on pre-existing code in a file the task happened to touch elsewhere).

# Findings

1. `crates/swissarmyhammer-validators/src/review/probes.rs` — `render_probe_evidence` (near line 171). Cognitive complexity 24 exceeds the threshold of 15. Nested control flow (multiple loops, conditionals, branches) is hard to follow and maintain. Extract the inner loop that renders rows into a separate helper function. Suggested structure: early return for empty results, then iterate results and delegate row rendering to `render_result_rows(out, result)`.

2. `crates/swissarmyhammer-validators/src/review/probes.rs` — `run_probes` (near line 377). Parameter `probe_names: &[String]` should be `&[&str]` so callers can pass `&["foo", "bar"]` instead of requiring `&["foo".to_string(), "bar".to_string()]`. Update the function body to use `.as_str()` or forward the borrowed references. Check all call sites when changing this signature.

# Acceptance

- `render_probe_evidence` cognitive complexity below `COGNITIVE_COMPLEXITY_THRESHOLD` (15).
- `run_probes` takes `&[&str]`, all call sites updated.
- `cargo nextest run -p swissarmyhammer-validators` passes.
- `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings` clean. #bug #review