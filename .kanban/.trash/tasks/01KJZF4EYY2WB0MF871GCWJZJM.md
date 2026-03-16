---
position_column: done
position_ordinal: l2
title: 'Fix broken doctest in swissarmyhammer/src/lib.rs line 15: TemplateContext not found'
---
The doc-test at swissarmyhammer/src/lib.rs line 15 fails to compile. It references `swissarmyhammer::TemplateContext` but `TemplateContext` has been moved to `swissarmyhammer_config`. The doc example needs to either re-export `TemplateContext` from the `swissarmyhammer` crate or update the example to use the correct path (`swissarmyhammer_config::TemplateContext`). Error: E0433 failed to resolve: could not find `TemplateContext` in `swissarmyhammer`. #test-failure