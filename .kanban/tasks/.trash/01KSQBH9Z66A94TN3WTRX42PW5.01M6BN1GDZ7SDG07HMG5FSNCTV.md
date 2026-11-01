---
assignees:
- claude-code
depends_on:
- 01KSQBEAVG5FCXF3TT411A88Z7
- 01KSQBEQ4XMETVGMBTJW25BDAC
- 01KSQBF4J2WV5XDEVA5QZXY8TV
- 01KSQBFMECY2QGC545BRXGR3JT
- 01KSQBG2EW2HNHQ911SHN6G6YK
- 01KSQBGPHT216JC640GNAA5NRA
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffbe80
project: llama-coverage
title: Add a CI coverage gate for llama-agent so the bar can't regress
---
## DONE (2026-05-28)

Added a `llama-agent-coverage` CI job (`.github/workflows/ci.yml`) that measures crate-scoped line coverage and fails the build below a threshold.

### Why not `cargo llvm-cov --fail-under-lines`
That flag gates the workspace-wide TOTAL, which is ~30% here because `--package llama-agent` still instruments other path-dep crates (agent-client-protocol-extras, swissarmyhammer-agents, …) that the llama-agent suite barely touches. Meaningless as a llama-agent gate. Instead the job exports an LCOV and runs `scripts/llama_agent_gap_report.py` (the same scoped tool the baseline card used), now extended with `--fail-under` (crate floor) and `--critical FILE:PCT` (per-file floors). Backward-compatible: the no-flag and positional-needle invocations still work.

### Threshold (recorded + justified)
- **Crate floor: 80% line.** Achieved post-epic = **85.04%**; pre-epic baseline = **78.01%**. 80% sits above baseline (so it ratchets up and blocks regression to the pre-epic / 0-token-bug state) with ~5pt slack for model-availability variance — the real-model smoke tests use the small qwen-0.6B and skip on HF rate-limit; on the self-hosted runner the model is cached so they run, but the margin absorbs a transient skip. NOT 100% (invites gaming/brittle exclusions).
- **Per-file critical floors** (guard bug-prone modules harder than the average): `generation/budget.rs:100` (the extracted arithmetic — the bug's home, must never regress), `stopper/mod.rs:95`, `queue.rs:90`, `acp/translation.rs:90`, `chat_template.rs:80` (tool-call parsing).

### Exclusions (annotated, not silently dropped)
The real-model FFI decode loops (`generation/mod.rs`, `model.rs` load path) bind llama.cpp and are covered by the small-model smoke tests, not unit coverage — documented in the CI job comment; not separately gated. No 27B download: the suite is hardcoded to qwen-0.6B (`src/test_models.rs`); CI sets `LLAMA_N_GPU_LAYERS=0`.

### Gate demonstrated (the card's "test")
- PASS: real thresholds against the post-epic LCOV → python exit 0, "COVERAGE GATE PASSED".
- FAIL (crate): `--fail-under 95` → exit 1, "crate line coverage 85.04% < floor 95.00%".
- FAIL (critical): `--critical queue.rs:99` → exit 1, per-file violation reported.
This is a cleaner, deterministic equivalent of "delete a covered test and confirm the step fails" — the floor itself is moved instead, with no flaky model dependence.

### Acceptance criteria
- [x] CI fails when llama-agent coverage drops below the threshold (demonstrated, exit 1).
- [x] Threshold recorded + justified (80% = achieved 85.04% with margin, above 78.01% baseline).
- [x] Legitimate exclusions (real-model FFI) annotated + explained in the CI job comment.
- [x] Gate runs without the 27B model download (qwen-0.6B only, GPU layers off).