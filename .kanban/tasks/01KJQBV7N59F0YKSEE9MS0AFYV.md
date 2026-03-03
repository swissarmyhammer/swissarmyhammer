---
title: 'Fix llama-agent test: test_read_text_file_not_found panics with Backend already initialized'
position:
  column: done
  ordinal: a0
---
Test `integration::acp_read_file::acp_read_file_tests::test_read_text_file_not_found` in package `llama-agent` panics at `llama-agent/tests/integration/acp_read_file.rs:61:18` with error: `Failed to create model manager: LoadingFailed("Backend already initialized by external code")`. This is a test isolation issue where the llama.cpp backend is being initialized by a previous test and cannot be re-initialized. The test needs proper isolation or the backend initialization needs to be made idempotent. #test-failure