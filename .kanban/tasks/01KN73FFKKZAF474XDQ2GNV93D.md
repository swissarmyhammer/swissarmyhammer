---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9880
title: Add tests for TestConfig and test_config helpers (lib.rs)
---
swissarmyhammer-config/src/lib.rs:465-538\n\nCoverage: 0% (0/31 lines)\n\nUncovered lines: 466-538 (all instrumented lines)\n\nFunctions:\n- `TestConfig::from_environment()` (line 466)\n- `TestConfig::create_llama_config()` (line 478)\n- `TestConfig::create_claude_config()` (line 499)\n- `TestConfig::create_llama_agent_config()` (line 503)\n- `skip_if_claude_disabled()` (line 509)\n- `is_llama_enabled()` (line 517)\n- `is_claude_enabled()` (line 522)\n- `get_enabled_executors()` (line 528)\n\nThese are test utility helpers in `pub mod test_config`. They construct LlamaAgentConfig and ModelConfig for testing. Although they are test helpers themselves, they contain conditional logic (env var parsing, executor filtering) that should be validated. Test that `from_environment` reads SAH_TEST_CLAUDE correctly, that `create_llama_config` returns expected defaults, and that `get_enabled_executors` returns the right set based on env vars. #Coverage_Gap