---
position_column: done
position_ordinal: b7
title: 'Fix compilation errors in swissarmyhammer-tools: missing TextEmbedder trait import and private load_model access'
---
File: swissarmyhammer-tools/src/mcp/tools/shell/state.rs\n\n7 compilation errors:\n1. E0624: load_model() is private (lines 471, 563) - llama-embedding/src/model.rs:129 defines it as private\n2. E0599: embed_text() not found (lines 476, 587) - need to import llama_embedding::TextEmbedder trait\n3. E0282: type annotations needed (lines 470, 475, 589) - cascading from above errors\n\nFix: Add `use llama_embedding::TextEmbedder;` to imports, and make load_model() public or use the trait method instead. #test-failure