---
position_column: todo
position_ordinal: c1
title: 'ane-embedding: test_coreml_inference fails - embeddings return NaN values'
---
Integration test `test_coreml_inference` in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/ane-embedding/tests/coreml_test.rs:115` fails. The CoreML model produces NaN values for all embedding dimensions instead of valid floats. Output shows: First 5 values: [NaN, NaN, NaN, NaN, NaN]. The assertion 'Embedding should not be all zeros' panics because NaN values are detected. #test-failure