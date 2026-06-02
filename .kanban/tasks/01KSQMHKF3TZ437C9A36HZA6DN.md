---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc280
title: shell execute_command/get_lines tests deadlock under parallel nextest
---
## DONE (2026-05-28)

Root cause was NOT a shared global registry (the card's guess) — `ShellExecuteTool` state is per-instance `Arc<Mutex<ShellState>>` and tests already use isolated instances. The real cause: the test constructor `ShellExecuteTool::new_isolated()` called `ShellState::with_dir`, which **lazy-loads the real `qwen-embedding` model** in the background embedding worker the first time a command produces output. Every subprocess-spawning test triggered a real model load; under parallel `nextest` dozens loaded concurrently → contention → hung at the 300s slow-timeout (while all passed under `--test-threads=1`, where loads serialize one at a time).

Fix (uses the existing design, no band-aid): `new_isolated()` now injects a `model_embedding::mock::MockEmbedder` via the already-present `with_embedder` path, so tests never load the real model. The embedding write-path stays deterministic and model-free — correct for the execute/get-lines tests, which don't exercise semantic search. Did NOT add `#[serial]` or a global `--test-threads=1` (the card explicitly forbade the latter); the contention is removed at its source rather than serialized around.

Verification: `cargo nextest run -p swissarmyhammer-tools shell --test-threads=8` → **183 tests, 183 passed in 1.7s** (previously the execute_command/get_lines family hung at 300s under parallelism). Deterministic by construction now — there is no model load to contend.

Acceptance criteria:
- [x] Identified the real shared cost (real embedding-model load per test), not the guessed registry deadlock.
- [x] Fixed at the source (inject MockEmbedder), not by serializing tests.
- [x] No global `--test-threads=1`.
- [x] Full shell suite passes fast under 8-way parallelism.