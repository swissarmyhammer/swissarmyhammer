---
assignees:
- claude-code
position_column: review
position_ordinal: '8180'
title: 'Review OOM: serialize review pipelines process-wide (cap=1)'
---
## Symptom
A full parallel `review` in a large repo OOMs even a 512GB box. The prior load-once-corpus fix only deduped the corpus WITHIN one run; it did nothing across concurrent runs.

## Root cause (parallelism multiplier)
Each `mcp__sah__review` invocation builds its own heavy resources and nothing caps concurrency:
- embedder model loaded fresh per run (`review_op::default_embedder_factory`)
- full embedding corpus loaded fresh per run (`open_index_connection` + `load_all_embedded_chunks`)
- fresh ACP agent per run; `AgentPool` bounds workers WITHIN a run only

Parallel `review file`-per-file ⇒ dozens/hundreds of pipelines, each holding ~GB corpus + ~GB model ⇒ OOM.

## Fix
Add a process-global async semaphore (1 permit) around the review pipeline so only ONE review runs at a time. Each run still fans out internally across its worker pool, so throughput is preserved. Acquire the permit in `run_review_request` (the `spawn_blocking` entry in `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`) before building the runtime; release on completion.

#review #bug #memory