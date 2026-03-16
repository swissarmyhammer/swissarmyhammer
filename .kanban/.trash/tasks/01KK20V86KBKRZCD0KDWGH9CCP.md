---
position_column: todo
position_ordinal: c4
title: CoreML model conversion produces NaN output with palettize4 quantization
---
convert_qwen_embedding.py --quantize palettize4 fails at verification step. The model converts and saves successfully (308.7 MB .mlpackage), but when verifying seq_len=64, the output embedding contains NaN values. AssertionError at line 193: "seq_len=64: Output contains NaN values". Note: coremltools logged "overflow encountered in cast" warnings during the MIL default pipeline, which may be related to the NaN production. #test-failure