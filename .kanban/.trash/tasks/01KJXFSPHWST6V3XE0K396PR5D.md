---
position_column: done
position_ordinal: c0
title: 'Fix failing test: prompt_resolver::tests::test_prompt_resolver_loads_user_prompts'
---
In crate swissarmyhammer-prompts, the test `prompt_resolver::tests::test_prompt_resolver_loads_user_prompts` panics with: called `Result::unwrap()` on an `Err` value: Other { message: "Prompt 'test_prompt' not found" } at swissarmyhammer-prompts/src/prompt_resolver.rs:186:49 #test-failure