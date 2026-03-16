---
position_column: done
position_ordinal: b6
title: 'Fix E0282: type annotations needed at swissarmyhammer-treesitter/src/index.rs lines 702 and 1036'
---
At lines 702 and 1036 of `/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/swissarmyhammer-treesitter/src/index.rs`, the compiler cannot infer the type of `model.load_model().await` and `model.embed_text(text).await`. These are downstream of the E0599 and E0624 errors -- fixing those will likely resolve these type inference failures. #test-failure