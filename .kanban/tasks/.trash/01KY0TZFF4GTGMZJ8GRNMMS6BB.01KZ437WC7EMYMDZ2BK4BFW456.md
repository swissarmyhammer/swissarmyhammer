---
assignees:
- claude-code
position_column: todo
position_ordinal: a980
title: 'Flaky: review_real_model_e2e hangs after synthesis on spurious Qwen tool-call turns (~40%)'
---
crates/swissarmyhammer-agent/tests/review_real_model_e2e.rs — review_runs_over_acp_against_a_real_local_model

INTERMITTENT real-model flake, not a deterministic failure: observed 2 timeouts then 3 consecutive passes (52s/23s/18s) on 2026-07-20 while verifying ^jn2wjd5 (review progress notifications).

Failure signature (from a RUST_LOG=info --no-capture run):
- Model loads from HF cache in ms; the review pipeline runs end to end; `review synthesis complete` logs at ~8s into the test (tasks_attempted=1 tasks_failed=1 — Qwen3-0.6B answered the fan-out with a spurious tool call, then think-text; findings JSON unparseable).
- After `run_pipeline_in_connection` returns, `run_review_over_agent`'s `connect_with` in crates/swissarmyhammer-validators/src/review/drive.rs never resolves; the test sits idle until nextest's 480s terminate-after kills it (TIMEOUT).

Matches the known Qwen3-0.6B spurious-tool-call flake class (~35%) and the ACP teardown/agent-client request hazards. Suspect: the spurious tool-call turn leaves a pending agent-side request or an un-drained turn that keeps the llama-agent connection dispatch loop alive after the pipeline closure completes.

Separate environment note: first two timeouts of the day were HF downloads being extremely slow (model not in cache; download exceeded 480s). Warm cache eliminates that mode.

What was tried: warmed the HF cache (`hf download unsloth/Qwen3-0.6B-GGUF Qwen3-0.6B-IQ4_NL.gguf`), reran 5x — 2 hangs (both post-synthesis), 3 passes. No code changed; test-gate run only. #test-failure