# Update Tests to Use Small Test Models Instead of Mocks

## Problem

The test infrastructure currently uses environment variables (`MOCK_ML_TESTS`, `RUN_ML_TESTS`, `SKIP_ML_TESTS`) to control whether tests use mock implementations or skip ML tests entirely. This violates the coding standard of never using mocks for model calls. Instead, tests should always use real models but with small, fast test models as defined in swissarmyhammer-config.

## Current Mock Logic

File: `swissarmyhammer-cli/tests/e2e_workflow_tests.rs`
- Lines 37-55: `should_run_expensive_ml_tests()` - Skips tests based on environment variables
- Lines 58-66: `should_use_mock_ml_operations()` - Forces mock usage instead of real models

Current logic:
- Uses mocks if `MOCK_ML_TESTS` is set
- Uses mocks by default when not running expensive tests
- Skips tests entirely in CI unless `RUN_ML_TESTS` is set

## Required Changes

1. **Remove mock logic**: Delete `should_use_mock_ml_operations()` function
2. **Always use real models**: Configure tests to always use small test models
3. **Update test configuration**: Use recommended test model configuration
4. **Remove environment variable dependencies**: Tests should run consistently everywhere
5. **Update model cache**: Configure for small model downloads instead of skipping

## Recommended Test Model Configuration

Based on memo "LlamaAgent Test Model Configuration":

```yaml
model:
  source:
    HuggingFace:
      repo: "unsloth/Qwen3-1.7B-GGUF"  
      filename: "Qwen3-1.7B-UD-Q6_K_XL.gguf"
  batch_size: 256
  use_hf_params: true
  debug: true
```

Alternative smaller model:
```yaml  
model:
  source:
    HuggingFace:
      repo: "unsloth/Phi-4-mini-instruct-GGUF"
      filename: "Phi-4-mini-instruct-Q4_K_M.gguf" 
  batch_size: 256
  use_hf_params: true
  debug: true
```

## Implementation Strategy

### Update Test Configuration

```rust
// Remove mock logic, always use small test models
fn get_test_llama_config() -> LlamaAgentConfig {
    LlamaAgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "unsloth/Qwen3-1.7B-GGUF".to_string(),
                filename: Some("Qwen3-1.7B-UD-Q6_K_XL.gguf".to_string()),
            },
            batch_size: 256,
            use_hf_params: true,
            debug: true,
        },
        mcp_server: McpServerConfig {
            port: 0,
            timeout_seconds: 30,
        },
        repetition_detection: Default::default(),
    }
}

#[tokio::test]
async fn test_workflow_with_real_small_model() {
    let config = get_test_llama_config();
    // Test with real model - small and fast for CI
    let executor = LlamaAgentExecutor::new(config);
    executor.initialize().await.unwrap();
    
    // Execute real workflow with small model
    let result = executor.execute_prompt(
        system_prompt,
        user_prompt, 
        &context,
        Duration::from_secs(60) // Reasonable timeout for small model
    ).await.unwrap();
    
    // Validate real model response
    assert!(result.is_object());
}
```

### Update Environment Variable Logic

```rust
// Replace mock/skip logic with fast model configuration
fn get_model_config_for_tests() -> ModelConfig {
    // Always use real models, but small/fast ones for tests
    if is_ci_environment() {
        // Use fastest model for CI
        ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "unsloth/Phi-4-mini-instruct-GGUF".to_string(),
                filename: Some("Phi-4-mini-instruct-Q4_K_M.gguf".to_string()),
            },
            batch_size: 128, // Smaller batch for CI
            use_hf_params: true,
            debug: false,
        }
    } else {
        // Use slightly larger model for local development testing
        ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "unsloth/Qwen3-1.7B-GGUF".to_string(), 
                filename: Some("Qwen3-1.7B-UD-Q6_K_XL.gguf".to_string()),
            },
            batch_size: 256,
            use_hf_params: true,
            debug: true,
        }
    }
}
```

## Benefits

- Tests actual model integration instead of mock behavior  
- Catches real issues with model loading, inference, and error handling
- Consistent behavior across all environments (local, CI, etc.)
- Fast execution with small models (~1.7B parameters)
- Eliminates environment variable complexity
- Follows coding standards requiring real implementations

## Files to Update

- `swissarmyhammer-cli/tests/e2e_workflow_tests.rs` - Remove mock logic
- `swissarmyhammer-config/src/lib.rs` - Update test configuration defaults  
- `swissarmyhammer-config/src/agent.rs` - Update `for_testing()` methods
- Any other test files using `should_use_mock_ml_operations()`

## Environment Variables to Remove/Change

- Remove dependency on: `MOCK_ML_TESTS`
- Keep but change: `RUN_ML_TESTS` → `RUN_LARGE_MODEL_TESTS` (for optional large model testing)
- Keep: `SKIP_ML_TESTS` (for environments where model download isn't possible)
- Keep: `SWISSARMYHAMMER_MODEL_CACHE` (for model caching)

## Acceptance Criteria

- [ ] `should_use_mock_ml_operations()` function removed
- [ ] All tests use real small models by default
- [ ] No mock implementations in model execution path
- [ ] Test configuration uses recommended small models
- [ ] Tests run consistently in CI and local environments  
- [ ] Fast execution with small models (< 2GB models)
- [ ] Model cache properly configured for small model downloads
- [ ] Environment variable complexity reduced
## Proposed Solution

After analyzing the current codebase, I can see that the main issue is in the `e2e_workflow_tests.rs` file which contains mock logic that violates our coding standards. Here's my implementation plan:

### 1. Current State Analysis
- `should_use_mock_ml_operations()` function forces mock usage based on environment variables
- Mock implementations exist for search indexing and query operations
- Tests skip real ML operations in CI by default
- `LlamaAgentConfig::for_testing()` already uses small models but tests still use mocks

### 2. Implementation Steps
1. **Remove Mock Functions**: Delete `should_use_mock_ml_operations()`, `mock_search_index()`, and `mock_search_query()` functions
2. **Update Test Logic**: Replace mock conditionals with direct calls to real implementations using small models
3. **Simplify Environment Logic**: Remove `MOCK_ML_TESTS` dependency, keep only `SKIP_ML_TESTS` for optional skipping
4. **Update Model Configuration**: Ensure `LlamaAgentConfig::for_testing()` uses the recommended Qwen3-1.7B model from the memo
5. **Test All Paths**: Verify all tests use real small models consistently

### 3. Benefits of This Approach
- Tests will catch real integration issues with model loading and inference
- Consistent behavior across all environments (no more CI-specific mocking)
- Fast execution with small 1.7B parameter model (~1GB download)
- Eliminates environment variable complexity
- Follows coding standards requiring real implementations

### 4. Risk Mitigation
- Keep timeout protections to handle infrastructure issues
- Maintain graceful degradation when model download fails
- Use persistent model cache to avoid repeated downloads
- Small model size ensures reasonable CI execution time
## Implementation Progress

### ✅ Completed Changes

1. **Updated Test Model Configuration**
   - Changed `DEFAULT_TEST_LLM_MODEL_REPO` from `"unsloth/Phi-4-mini-instruct-GGUF"` to `"unsloth/Qwen3-1.7B-GGUF"`
   - Changed `DEFAULT_TEST_LLM_MODEL_FILENAME` from `"Phi-4-mini-instruct-Q4_K_M.gguf"` to `"Qwen3-1.7B-UD-Q6_K_XL.gguf"`
   - This matches the memo recommendation for optimal test model size and performance

2. **Removed Mock Logic from e2e_workflow_tests.rs**
   - ❌ Deleted `should_use_mock_ml_operations()` function
   - ❌ Deleted `mock_search_index()` function  
   - ❌ Deleted `mock_search_query()` function
   - ❌ Deleted `should_run_expensive_ml_tests()` function (unused)
   - ✅ Updated `try_search_index()` to always use real implementations
   - ✅ Updated `try_search_query()` to always use real implementations

3. **Simplified Environment Variable Logic**
   - ❌ Removed dependency on `MOCK_ML_TESTS` environment variable
   - ✅ Kept `SKIP_ML_TESTS` for completely skipping ML tests when needed
   - ✅ Removed CI-specific mock behavior - tests now run consistently everywhere

4. **Updated Test Documentation**
   - Updated all test comments to reflect real model usage instead of mocks
   - Updated timeout handling messages to reference "small models" instead of "mock/real modes"
   - Updated performance expectations to reflect real small model behavior

### ✅ Verification Results

- **Tests Pass**: All tests in the e2e_workflow_tests.rs file are passing
- **No Mock References**: Confirmed no remaining references to `should_use_mock_ml_operations()` or `MOCK_ML_TESTS`
- **Real Model Usage**: Tests now always use the Qwen3-1.7B model for authentic integration testing
- **Consistent Behavior**: Same behavior in CI and local environments - no more environment-specific mocking

### ✅ Benefits Achieved

1. **Authentic Testing**: Tests now catch real integration issues with model loading and inference
2. **Simplified Logic**: Removed complex environment variable decision trees
3. **Fast Execution**: Small 1.7B model ensures reasonable test execution time (~1.2GB download)
4. **Consistent Behavior**: No more surprises between CI and local test environments
5. **Standards Compliance**: Now follows coding standards requiring real implementations

### Environment Variables After Changes

- ❌ `MOCK_ML_TESTS` - **REMOVED** (no longer needed)
- ❌ `RUN_ML_TESTS` - **NOT NEEDED** (tests always run with real small models by default)  
- ✅ `SKIP_ML_TESTS` - **KEPT** (allows completely skipping ML tests when model download isn't possible)
- ✅ `SWISSARMYHAMMER_MODEL_CACHE` - **KEPT** (for model caching optimization)
## Code Review Completion - All Issues Fixed ✅

### Summary of Changes Made

Successfully completed all items identified in the code review. The refactor to use real small models instead of mocks is now fully consistent across all test files.

### Issues Resolved

1. **✅ Fixed Inconsistent Test Model Configuration**
   - Updated `llama_test_config.rs` to use `DEFAULT_TEST_LLM_MODEL_REPO` and `DEFAULT_TEST_LLM_MODEL_FILENAME` constants
   - Replaced hardcoded Phi-4-mini references with Qwen3-1.7B model constants

2. **✅ Fixed Duplicate Model Configuration** 
   - Updated all hardcoded model references in `development()` and `ci()` methods
   - Now consistently uses shared constants from lib.rs

3. **✅ Updated Test Assertions**
   - Fixed test assertions in `agent.rs` to expect Qwen3-1.7B instead of Phi-4-mini
   - Updated both config and test files to use the same model consistently

4. **✅ Removed Debug Print Statement**
   - Removed debug println from `LlamaAgentConfig::for_testing()` method
   - Clean production code without debug artifacts

5. **✅ Added Missing Imports**
   - Added proper imports for `DEFAULT_TEST_LLM_MODEL_REPO` and `DEFAULT_TEST_LLM_MODEL_FILENAME` constants
   - All files now reference centralized constants

6. **✅ Additional Fixes Found and Resolved**
   - Updated documentation comment in lib.rs to reference Qwen3-1.7B instead of Phi-4-mini
   - Fixed test configuration in `agent_config_file_loading_tests.rs` TOML and YAML examples
   - Fixed remaining test assertions in `llama_test_config.rs` tests

### Verification Results

- **✅ Build Success**: `cargo build --package swissarmyhammer-config` completed successfully
- **✅ Tests Pass**: All 46 tests in swissarmyhammer-config package pass
- **✅ No References**: Confirmed zero remaining references to "Phi-4-mini" across the codebase
- **✅ Consistency**: All test files now use the same Qwen3-1.7B model configuration

### Benefits Achieved

1. **Authentic Testing**: Tests now use real small models instead of mocks, catching real integration issues
2. **Consistent Configuration**: All test files use the same centralized model constants
3. **Clean Code**: Removed debug artifacts and hardcoded values
4. **Standards Compliance**: Now follows coding standards requiring real implementations
5. **Fast Execution**: Small 1.7B model ensures reasonable test execution time (~1.2GB download)

The refactoring is now complete and ready for use. All tests run with real small models consistently across all environments.