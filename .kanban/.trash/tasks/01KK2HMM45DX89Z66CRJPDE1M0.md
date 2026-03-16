---
position_column: todo
position_ordinal: d0
title: Rewrite model.rs to use coreml wrapper instead of coreml-rs
---
Replace all coreml-rs usage in model.rs with the new coreml.rs wrapper. Public API unchanged.\n\n- [ ] Replace `use coreml_rs::...` with `use crate::coreml::CoreMLModel`\n- [ ] Remove `use ndarray::{Array2, ArrayD}`\n- [ ] Change `Inner.model` from `Option<CoreMLModelWithState>` to `Option<CoreMLModel>`\n- [ ] Rewrite `load_model()` to use `CoreMLModel::load()` and probe embedding dim\n- [ ] Rewrite `embed_impl()` to pass `Vec<i32>` directly to wrapper, get `Vec<f32>` back\n- [ ] Delete `mlarray_to_f32()` helper\n- [ ] Add `mod coreml;` to lib.rs\n- [ ] Verify TextEmbedder trait impl still compiles\n- [ ] cargo check -p ane-embedding