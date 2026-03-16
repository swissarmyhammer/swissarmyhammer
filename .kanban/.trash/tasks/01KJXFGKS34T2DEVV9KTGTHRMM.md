---
position_column: done
position_ordinal: b8
title: 'Fix failing unit test: prompt_resolver::tests::test_prompt_resolver_loads_user_prompts'
---
File: swissarmyhammer-prompts/src/prompt_resolver.rs:186\n\nPanicked with: called Result::unwrap() on an Err value: Other { message: \"Prompt 'test_prompt' not found\" }\n\nThe test expects a user prompt named 'test_prompt' to be loaded but the resolver cannot find it. #test-failure