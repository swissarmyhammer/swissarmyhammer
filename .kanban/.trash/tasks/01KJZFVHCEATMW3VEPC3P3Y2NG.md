---
position_column: done
position_ordinal: m1
title: 'Fix 18 failing doc-tests in swissarmyhammer-prompts: unresolved crate `swissarmyhammer`'
---
All 18 doc-tests in swissarmyhammer-prompts/src/prompts.rs fail with: error[E0433]: failed to resolve: use of unresolved module or unlinked crate `swissarmyhammer`. The doc examples use `use swissarmyhammer::{Prompt, common::{Parameter, ParameterType}};` which cannot resolve in the doc-test context. Either add swissarmyhammer as a dev-dependency of swissarmyhammer-prompts, rewrite the doc examples to use `use swissarmyhammer_prompts::...` directly, or mark them as `no_run`. #test-failure