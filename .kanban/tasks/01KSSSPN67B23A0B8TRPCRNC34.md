---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffca80
title: 'KV-cache reuse: validate prefix by content fingerprint, not just length'
---
Follow-up from review of d4a69cbe8 (card 01KSSS5H82YC0TX0CM6SQV8CRP, finding #3).

`queue.rs::streaming_offset_decision` (and the batch path's `compute_template_token_count`) gate KV-cache reuse on a LENGTH check (`offset >= total` → discard). This is a length check standing in for a prefix check. It is correct only under the current invariant (conversation grows append-only; compaction strictly shrinks the prompt and never rewrites a retained prefix while keeping a similar token count). Documented in `prepare_streaming_kv_cache` doc comment.

Robust fix: store the cached prompt's token sequence (or a rolling hash per position) alongside the state bytes in `SessionStateCache`, and on restore compute the longest common prefix between the cached tokens and the new prompt's tokenization. Reuse only that common-prefix length (trimming the KV beyond it via `clear_kv_cache_seq` / `llama_memory_seq_rm`), instead of trusting the full cached length. This makes reuse correct under ANY prompt mutation (compaction rewrite, edited history) and is belt-and-suspenders for partial/disconnect saves.

This is the design llama.cpp's server slot-cache uses. Not blocking today, but required before raising worker count or adding history-rewriting features.