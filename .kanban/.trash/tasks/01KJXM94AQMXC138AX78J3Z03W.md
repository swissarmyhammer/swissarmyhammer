---
position_column: done
position_ordinal: f5
title: 'Fix swissarmyhammer doctest: TemplateContext not found in swissarmyhammer crate'
---
The doc-test in `swissarmyhammer/src/lib.rs` (line 15) fails to compile: error[E0433]: could not find `TemplateContext` in `swissarmyhammer`. The doc example references `swissarmyhammer::TemplateContext::new()` but TemplateContext is in `swissarmyhammer_config`, not re-exported from the root crate.