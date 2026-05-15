---
due: ''
position_column: done
position_ordinal: ffffeb80
title: 'Fix failing test: prompt_resolver::tests::test_prompt_resolver_loads_user_prompts'
---
In crate swissarmyhammer-prompts, the test `test_prompt_resolver_loads_user_prompts` panics at swissarmyhammer-prompts/src/prompt_resolver.rs:186:49 with: called `Result::unwrap()` on an `Err` value: Other { message: \"Prompt 'test_prompt' not found\" }. Rerun with: cargo test -p swissarmyhammer-prompts --lib #test-failure