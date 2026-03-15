---
position_column: done
position_ordinal: t7
title: 'Fix ane-embedding integration test: test_load_model - missing mlpackage file'
---
test_load_model panics at ane-embedding/tests/integration_test.rs:41 with: Failed to load model: Backend(ModelLoader(InvalidConfig("Path is not a file: .../var/data/models/qwen3-embedding-0.6b/Qwen3-Embedding-0.6B.mlpackage"))). The model file is missing from the expected path.