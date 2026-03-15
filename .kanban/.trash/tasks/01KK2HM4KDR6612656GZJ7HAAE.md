---
position_column: todo
position_ordinal: c8
title: Create src/coreml.rs wrapper module for objc2-core-ml
---
Thin internal module isolating all unsafe ObjC interop from business logic.\n\n- [ ] Add `CoreMLModel` struct wrapping `Retained<MLModel>`\n- [ ] Implement `load(path: &Path) -> Result<Self>` using MLModelConfiguration + CPUAndNeuralEngine\n- [ ] Implement `predict_embedding(&self, input_ids: &[i32], attention_mask: &[i32], seq_len: usize) -> Result<PredictionOutput>`\n- [ ] Create MLMultiArray with MLMultiArrayDataType::Int32 (the bug fix)\n- [ ] Write data via getMutableBytesWithHandler + ptr::copy_nonoverlapping\n- [ ] Build MLDictionaryFeatureProvider, run prediction\n- [ ] Extract output as Vec<f32>, handle both f32 and f16 output types\n- [ ] Wrap prediction body in autoreleasepool\n- [ ] Map NSError to EmbeddingError::CoreML(message)\n- [ ] Implement embedding_dim() to probe output shape\n- [ ] cargo check -p ane-embedding