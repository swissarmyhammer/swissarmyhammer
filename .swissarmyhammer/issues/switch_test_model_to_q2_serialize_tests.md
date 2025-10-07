# Switch Test Model to Q2 and Serialize LLM Tests

## Problem

Currently 12 LlamaAgent tests in `swissarmyhammer-agent-executor` and 2 tests in `swissarmyhammer-workflow` are marked `#[ignore]` because they require loading a real LLM model. The tests use:

**Current Model:** `unsloth/Qwen3-4B-Instruct-2507-GGUF/Qwen3-4B-Instruct-2507-UD-Q4_K_XL.gguf`

The ignore reason states: "hangs during real server initialization - requires real model files"

However, the **actual problem** isn't that models can't load - it's that:
1. **Multiple tests try to load models simultaneously** → memory exhaustion/hangs
2. **Q4 quantization** (4-bit) is unnecessarily large for testing
3. Tests aren't serialized, so they compete for resources

## Affected Tests

### swissarmyhammer-agent-executor (12 tests)
All in `src/llama/executor.rs`:
- Line 955: `test_llama_agent_executor_initialization`
- Line 985: `test_llama_agent_executor_double_initialization`
- Line 1072: `test_llama_agent_executor_initialization_with_validation`
- Line 1122: `test_llama_agent_executor_global_management`
- Line 1169: `test_llama_agent_executor_execute_with_init`
- Line 1226: `test_llama_agent_executor_random_port`
- Line 1249: `test_llama_agent_executor_drop_cleanup`
- Line 1271: `test_http_mcp_server_integration`
- Line 1501: `test_wrapper_singleton_behavior`
- Line 1568: `test_wrapper_execute_with_init`
- (2 more in the file)

### swissarmyhammer-workflow (2 tests)
In `src/actions.rs`:
- Line 2286: `test_executor_factory_llama_agent`
- Line 3421: `test_agent_executor_factory_llama_agent`

**Total: 14 ignored tests**

## Solution

### 1. Switch to Q2 Quantization

Change the default test model in `swissarmyhammer-config/src/lib.rs`:

```rust
// BEFORE (line 368-376):
pub const DEFAULT_TEST_LLM_MODEL_REPO: &str = "unsloth/Qwen3-4B-Instruct-2507-GGUF";
pub const DEFAULT_TEST_LLM_MODEL_FILENAME: &str = "Qwen3-4B-Instruct-2507-UD-Q4_K_XL.gguf";

// AFTER:
pub const DEFAULT_TEST_LLM_MODEL_REPO: &str = "unsloth/Qwen3-4B-Instruct-2507-GGUF";
pub const DEFAULT_TEST_LLM_MODEL_FILENAME: &str = "Qwen3-4B-Instruct-2507-Q2_K.gguf";
```

**Benefits of Q2:**
- **~60% smaller file size** (2-bit vs 4-bit)
- **Faster model loading** (less data to read)
- **Lower memory footprint** (critical for parallel test execution)
- **Sufficient for test validation** (we're testing functionality, not quality)

### 2. Serialize All LLM Tests

Add `#[serial]` attribute to ensure only one test loads a model at a time:

```rust
// BEFORE:
#[tokio::test]
#[ignore = "hangs during real server initialization - requires real model files"]
async fn test_llama_agent_executor_initialization() {
    // ...
}

// AFTER:
#[tokio::test]
#[serial_test::serial] // Ensure only one LLM test runs at a time
async fn test_llama_agent_executor_initialization() {
    // ...
}
```

**Remove all `#[ignore]` attributes** from these 14 tests.

### 3. Verify serial_test Dependency

Check that `serial_test` is in `Cargo.toml` dev-dependencies:

```toml
[dev-dependencies]
serial_test = "3.2"  # or current version
```

Already present in:
- `swissarmyhammer-agent-executor/Cargo.toml`
- `swissarmyhammer-workflow/Cargo.toml`

## Implementation Plan

### Step 1: Verify Q2 Model Availability

Check that the Q2 model exists on HuggingFace:
```bash
# Visit: https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF
# Confirm Q2_K quantization is available
```

**Alternative if Q2 not available:** Use smallest available quantization (Q3_K_S or Q4_K_S)

### Step 2: Update Model Constants

File: `swissarmyhammer-config/src/lib.rs`

```rust
// Lines 368-376
pub const DEFAULT_TEST_LLM_MODEL_REPO: &str = "unsloth/Qwen3-4B-Instruct-2507-GGUF";
pub const DEFAULT_TEST_LLM_MODEL_FILENAME: &str = "Qwen3-4B-Instruct-2507-Q2_K.gguf";
```

### Step 3: Update All Ignored Tests

**For `swissarmyhammer-agent-executor/src/llama/executor.rs`:**

Find all tests with:
```rust
#[ignore = "hangs during real server initialization - requires real model files"]
```

Replace with:
```rust
#[serial_test::serial] // Only one LLM test at a time
```

Keep existing `#[tokio::test]` and any other attributes.

**For `swissarmyhammer-workflow/src/actions.rs`:**

Same replacement for lines 2286 and 3421.

### Step 4: Add Documentation Comments

Add comment explaining serialization:

```rust
#[tokio::test]
#[serial_test::serial] // LLM tests must run serially to avoid memory exhaustion
async fn test_llama_agent_executor_initialization() {
    // Test loads real LLM model - runs serially to manage memory
    // ...
}
```

### Step 5: Test Execution

**Validate changes:**

```bash
# Run all LLM tests serially (should now work)
cargo test --package swissarmyhammer-agent-executor llama -- --ignored

# Run all workflow tests
cargo test --package swissarmyhammer-workflow executor_factory

# Full test suite (LLM tests will run serially)
cargo test --workspace
```

**Expected behavior:**
- Tests no longer hang
- Tests run one at a time (slower, but reliable)
- All 14 tests pass
- No `#[ignore]` needed

## Alternative: Conditional Test Execution

If Q2 model + serialization still too slow, add environment flag:

```rust
#[tokio::test]
#[serial_test::serial]
async fn test_llama_agent_executor_initialization() {
    // Skip if explicitly disabled
    if std::env::var("SAH_SKIP_LLM_TESTS").is_ok() {
        eprintln!("Skipping LLM test (SAH_SKIP_LLM_TESTS set)");
        return;
    }
    
    // Test code...
}
```

Allow CI/users to skip with:
```bash
export SAH_SKIP_LLM_TESTS=1
cargo test --workspace
```

## Files to Modify

1. `swissarmyhammer-config/src/lib.rs` (lines 368-376) - Update model constants
2. `swissarmyhammer-agent-executor/src/llama/executor.rs` - Remove `#[ignore]`, add `#[serial_test::serial]` to 12 tests
3. `swissarmyhammer-workflow/src/actions.rs` - Remove `#[ignore]`, add `#[serial_test::serial]` to 2 tests
4. Optionally update docs/comments explaining serialization

## Success Criteria

- [ ] Model constant changed to Q2 quantization (or smallest available)
- [ ] All 14 `#[ignore]` attributes removed
- [ ] All 14 tests have `#[serial_test::serial]` attribute
- [ ] `serial_test` dependency verified in both crates
- [ ] Tests pass when run individually: `cargo test test_llama_agent_executor_initialization`
- [ ] Tests pass when run together: `cargo test --package swissarmyhammer-agent-executor`
- [ ] No hangs or memory exhaustion
- [ ] Documentation comments added explaining serialization requirement

## Estimated Effort

- Model availability check: 5 minutes
- Code changes: 30 minutes (straightforward find/replace)
- Testing & validation: 30-60 minutes (model download + test runs)
- **Total: 1-1.5 hours**

## Benefits

1. **Test coverage restored:** 14 tests no longer skipped
2. **Faster test execution:** Smaller model loads faster
3. **Lower memory usage:** Q2 quantization uses less RAM
4. **CI-friendly:** Tests can run in CI with serialization
5. **No infrastructure changes:** Uses existing `serial_test` crate

## Risks & Mitigation

**Risk:** Q2 model might not exist for this repo
**Mitigation:** Check HuggingFace first; use Q3_K_S or Q4_K_S as fallback

**Risk:** Serial execution makes test suite slower
**Mitigation:** Only affects 14 tests; acceptable tradeoff for coverage

**Risk:** Tests might still hang on low-memory systems
**Mitigation:** Add `SAH_SKIP_LLM_TESTS` environment variable escape hatch

## Notes

The key insight: **Tests don't fail because models can't load - they fail because MULTIPLE models try to load simultaneously.**

Serialization ensures only one model is in memory at a time, allowing all tests to pass reliably.
