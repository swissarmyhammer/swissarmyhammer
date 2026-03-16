---
position_column: done
position_ordinal: h6
title: 'Refactor ModelConfig: strip llama-specific fields from model-loader'
---
Remove 5 llama-specific fields (batch_size, n_seq_max, n_threads, n_threads_batch, use_hf_params) from model-loader ModelConfig. Create local ModelConfig in llama-agent with all fields. Update all consumers.\n\nSteps:\n1. Strip model-loader/src/types.rs ModelConfig\n2. Define ModelConfig in llama-agent/src/types/configs.rs\n3. Update llama-agent/src/types/mod.rs re-exports\n4. Update llama-agent/src/model.rs resolver call\n5. Update llama-embedding/src/model.rs\n6. Update ane-embedding/src/model.rs\n7. Update model-loader tests and examples\n8. Verify all builds and tests pass