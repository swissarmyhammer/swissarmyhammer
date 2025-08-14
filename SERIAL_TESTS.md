# Serial Tests

This document lists all tests that use `#[serial_test::serial]` to prevent race conditions and ensure isolation.

## Config Tests
- **`swissarmyhammer/src/config.rs`**:
  - `test_config_with_env_vars` - Tests environment variable configuration

## SAH Config Tests  
- **`swissarmyhammer/src/sah_config/loader.rs`**:
  - `test_environment_variable_substitution` - Tests environment variable substitution in config

## ~~Memoranda Storage Tests~~ (CONVERTED TO PARALLEL)
- **`swissarmyhammer/src/memoranda/storage.rs`**: ✅ **All storage tests converted to use `IsolatedTestHome` and run in parallel**
  - ~~`test_directory_creation`~~ - Now uses `IsolatedTestHome` for directory isolation
  - ~~`test_readonly_directory_error_handling`~~ - Now uses `IsolatedTestHome` for permission test isolation

## ~~Search Embedding Tests~~ (CONVERTED TO PARALLEL)
- **`swissarmyhammer/src/search/embedding.rs`**: ✅ **All embedding tests converted to use `IsolatedTestHome` and run in parallel**
  - ~~`test_embedding_engine_creation`~~ - Now uses `IsolatedTestHome` 
  - ~~`test_embedding_engine_with_model_id`~~ - Now uses `IsolatedTestHome`
  - ~~`test_embedding_engine_invalid_config`~~ - Now uses `IsolatedTestHome`
  - ~~`test_embed_text`~~ - Now uses `IsolatedTestHome`
  - ~~`test_embed_text_empty`~~ - Now uses `IsolatedTestHome`
  - ~~`test_embed_chunk`~~ - Now uses `IsolatedTestHome`
  - ~~`test_embed_batch`~~ - Now uses `IsolatedTestHome`
  - ~~`test_semantic_consistency`~~ - Now uses `IsolatedTestHome`
  - ~~`test_model_info`~~ - Now uses `IsolatedTestHome`
  - ~~`test_prepare_chunk_text`~~ - Now uses `IsolatedTestHome`
  - ~~`test_clean_text`~~ - Now uses `IsolatedTestHome`
  - ~~`test_clean_text_truncation`~~ - Now uses `IsolatedTestHome`

## CLI Validation Tests
- **`swissarmyhammer-cli/src/validate.rs`**:
  - `test_validate_command_loads_same_workflows_as_flow_list` - Tests consistency between validate and flow list
  - `test_validate_all_workflows_integration` - Integration test for workflow validation

## ~~Workflow Sub-workflow State Pollution Tests~~ (CONVERTED TO PARALLEL)
- **`swissarmyhammer/src/workflow/actions_tests/sub_workflow_state_pollution_tests.rs`**: ✅ **All workflow state pollution tests converted to use `IsolatedTestHome` and run in parallel**
  - ~~`test_nested_workflow_state_name_pollution`~~ - Now uses `IsolatedTestHome` for state isolation
  - ~~`test_nested_workflow_correct_action_execution`~~ - Now uses `IsolatedTestHome` for execution isolation  
  - ~~`test_deeply_nested_workflows_state_isolation`~~ - Now uses `IsolatedTestHome` for nested state isolation

## Notes

- These tests are marked with `#[serial_test::serial]` to prevent race conditions
- Most of these tests manipulate global state (environment variables, file system, model loading)
- Serial tests should be used sparingly and only when absolutely necessary for test correctness

## Recent Improvements

### ✅ Workflow Executor Abort Tests (CONVERTED TO PARALLEL)
- **`swissarmyhammer/src/workflow/executor/tests.rs`**: All abort-related tests now use `IsolatedTestEnvironment` (combines HOME isolation + current working directory isolation) instead of `#[serial_test::serial]`
- This prevents abort file pollution between tests while allowing parallel execution

### ✅ Search Embedding Tests (CONVERTED TO PARALLEL)  
- **`swissarmyhammer/src/search/embedding.rs`**: All embedding tests converted to use `IsolatedTestHome` and run in parallel
- These tests use mock embedding engines and don't need serialization
- **Performance improvement**: 13 tests now run in parallel (~0.57s) vs serial execution

### ✅ Memoranda Storage Tests (CONVERTED TO PARALLEL)
- **`swissarmyhammer/src/memoranda/storage.rs`**: 2 storage tests converted to use `IsolatedTestHome` and run in parallel
- These tests create temporary directories and modify file permissions, now safely isolated
- **Performance improvement**: 48 total storage tests run in parallel (~1.2s)

### ✅ Workflow Sub-workflow State Pollution Tests (CONVERTED TO PARALLEL)
- **`swissarmyhammer/src/workflow/actions_tests/sub_workflow_state_pollution_tests.rs`**: 3 workflow state tests converted to use `IsolatedTestHome` and run in parallel  
- These tests use global `TEST_STORAGE_REGISTRY` but each properly cleans up after itself
- **Performance improvement**: 3 tests now run in parallel (~0.35s) vs serial execution

## Summary

**Before**: 25+ serial tests requiring sequential execution  
**After**: 13 embedding tests + 8 workflow executor abort tests + 2 storage tests + 3 workflow state tests = **26 tests converted to parallel execution**

This significantly improves test suite performance while maintaining test isolation and reliability.