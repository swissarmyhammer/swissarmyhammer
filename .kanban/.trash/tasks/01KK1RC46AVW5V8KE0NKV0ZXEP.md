---
position_column: done
position_ordinal: u1
title: 'Fix ane-embedding integration test: test_normalization - missing mlpackage file'
---
test_normalization panics at ane-embedding/tests/integration_test.rs:122 with: Failed to load model: Backend(ModelLoader(InvalidConfig("Path is not a file: .../var/data/models/qwen3-embedding-0.6b/Qwen3-Embedding-0.6B.mlpackage")))