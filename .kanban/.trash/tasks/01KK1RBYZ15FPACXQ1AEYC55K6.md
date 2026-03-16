---
position_column: done
position_ordinal: t8
title: 'Fix ane-embedding integration test: test_embed_single_text - missing mlpackage file'
---
test_embed_single_text panics at ane-embedding/tests/integration_test.rs:58 with: Failed to load model: Backend(ModelLoader(InvalidConfig("Path is not a file: .../var/data/models/qwen3-embedding-0.6b/Qwen3-Embedding-0.6B.mlpackage")))