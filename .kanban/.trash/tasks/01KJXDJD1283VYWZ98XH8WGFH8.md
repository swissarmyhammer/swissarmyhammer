---
position_column: done
position_ordinal: b4
title: 'Fix E0599: missing `use llama_embedding::TextEmbedder` import in swissarmyhammer-treesitter/src/index.rs'
---
Two calls to `model.embed_text()` at lines 587 and 1036 in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/swissarmyhammer-treesitter/src/index.rs` fail because the `TextEmbedder` trait is not in scope. Fix: add `use llama_embedding::TextEmbedder;` at line 27. #test-failure