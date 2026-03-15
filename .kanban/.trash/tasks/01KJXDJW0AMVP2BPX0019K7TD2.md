---
position_column: done
position_ordinal: b5
title: 'Fix E0624: private method `load_model` called in swissarmyhammer-treesitter/src/index.rs:702'
---
At line 702 of `/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/swissarmyhammer-treesitter/src/index.rs`, `model.load_model()` is called but `load_model` is a private method defined at `/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/llama-embedding/src/model.rs:129`. Either make `load_model` public or use the appropriate public API. #test-failure