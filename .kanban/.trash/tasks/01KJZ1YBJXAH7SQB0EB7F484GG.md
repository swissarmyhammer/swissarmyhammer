---
position_column: done
position_ordinal: h7
title: ModelConfig contains consumer-specific fields
---
**model-loader/src/types.rs**

`ModelConfig` still contains llama-specific fields like `batch_size`, `n_seq_max`, `n_threads`, `n_threads_batch`, `use_hf_params` that are meaningless to a runtime-agnostic resolver. The resolver only needs `source`, `retry_config`, and `debug`.

**Fix:**
- [ ] Remove consumer-specific fields from `ModelConfig` (batch_size, n_seq_max, n_threads, n_threads_batch, use_hf_params)
- [ ] Update consumers to carry these fields in their own config structs
- [ ] Verify tests pass #review-finding #warning