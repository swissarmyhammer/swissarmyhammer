---
position_column: todo
position_ordinal: c7
title: 'ane-embedding: test_coreml_inference returns all-zero embeddings'
---
The test `ane-embedding/tests/coreml_test.rs::test_coreml_inference` fails because the CoreML model (Qwen3-Embedding-0.6B-seq128.mlpackage) returns all-zero embeddings when run on ANE. The model loads fine (test_coreml_load_model passes), and the output shape is correct [1, 1024], but all values are 0.0. This is a CoreML/ANE hardware or model conversion issue, not a code bug. The assertion `sum.abs() > 0.0` correctly catches this. This needs investigation into the CoreML model conversion (palettization/quantization may be the root cause).