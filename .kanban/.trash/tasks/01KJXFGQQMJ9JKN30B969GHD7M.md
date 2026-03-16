---
position_column: done
position_ordinal: b9
title: 'Fix 18 failing doctests in swissarmyhammer-prompts: unresolved import swissarmyhammer'
---
File: swissarmyhammer-prompts/src/prompts.rs\n\nAll 18 doctests fail with: E0432 unresolved import `swissarmyhammer` - use of unresolved module or unlinked crate.\n\nThe doctests use `use swissarmyhammer::{PromptLibrary, Prompt}` but swissarmyhammer is not a dev-dependency of swissarmyhammer-prompts. Either add it as a dev-dependency or update the doc examples to use the correct crate path. #test-failure