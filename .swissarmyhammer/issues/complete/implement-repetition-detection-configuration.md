# Implement repetition detection configuration for llama-agent executor

## Description
The LlamaAgent executor has a TODO comment indicating that repetition detection configuration needs to be implemented.

**Location:** `swissarmyhammer/src/workflow/agents/llama_agent_executor.rs:817`

**Current code:**
```rust
// TODO: Implement repetition detection configuration
```

## Requirements
- Add configuration options for repetition detection in LlamaAgent
- Implement the detection logic to prevent infinite loops
- Add appropriate validation and error handling
- Document the configuration options

## Acceptance Criteria
- [ ] Configuration structure for repetition detection settings
- [ ] Implementation of repetition detection algorithm
- [ ] Integration with existing LlamaAgent executor
- [ ] Tests covering repetition scenarios
- [ ] Documentation for configuration options

## Proposed Solution

After analyzing the code, I found that:

1. SwissArmyHammer has a `RepetitionDetectionConfig` in `swissarmyhammer-config/src/agent.rs` with fields:
   - `enabled: bool` (default: true)
   - `repetition_penalty: f64` (default: 1.1) 
   - `repetition_threshold: usize` (default: 50)
   - `repetition_window: usize` (default: 64)

2. The llama-agent library supports repetition detection through `StoppingConfig::repetition_detection` using `RepetitionConfig` with fields:
   - `min_pattern_length: usize` (default: 10)
   - `max_pattern_length: usize` (default: 100)
   - `min_repetitions: usize` (default: 3)
   - `window_size: usize` (default: 1000)

The solution will:

1. **Map configurations**: Create a mapping function from SwissArmyHammer's `RepetitionDetectionConfig` to llama-agent's `RepetitionConfig`
2. **Integrate with StoppingConfig**: Use the mapped configuration in the `AgentConfig` creation
3. **Handle the enabled flag**: Only include repetition detection when enabled
4. **Add comprehensive tests**: Test the mapping and integration

### Mapping Strategy:
- `repetition_threshold` → `min_repetitions` (with validation)
- `repetition_window` → `window_size` 
- Use reasonable defaults for `min_pattern_length` and `max_pattern_length`
- `repetition_penalty` will be noted as unsupported (different paradigm)

### Implementation Steps:
1. Create mapping function in `llama_agent_executor.rs`
2. Integrate with `create_agent_config()` method
3. Add comprehensive tests
4. Update existing test configurations

## Implementation Complete ✅

The repetition detection configuration has been successfully implemented with the following changes:

### Changes Made

1. **Added imports**: Import `RepetitionConfig` and `StoppingConfig` from llama-agent
2. **Created mapping function**: `create_repetition_config()` converts SwissArmyHammer config to llama-agent format:
   - `repetition_threshold` → `min_repetitions` (with min value 2)
   - `repetition_window` → `window_size` (with min value 100)
   - Uses reasonable defaults for pattern length (10-100 chars)
   - Warns about unsupported `repetition_penalty` (different paradigm)

3. **Created stopping config function**: `create_stopping_config()` creates proper StoppingConfig with repetition detection
4. **Updated GenerationRequest creation**: Now uses custom StoppingConfig instead of default
5. **Comprehensive tests**: Added 5 new tests covering all scenarios

### Validation & Testing

- ✅ All existing tests pass (19/19)
- ✅ New tests pass (5/5) 
- ✅ Integration test passes
- ✅ Full project builds successfully

### Configuration Mapping

| SwissArmyHammer Config | llama-agent Config | Notes |
|------------------------|-------------------|-------|
| `enabled: bool` | Controls inclusion | Only creates config when enabled |
| `repetition_threshold` | `min_repetitions` | Ensures minimum value of 2 |
| `repetition_window` | `window_size` | Ensures minimum value of 100 |
| `repetition_penalty` | ⚠️ Not supported | Warns user - different paradigm |
| - | `min_pattern_length: 10` | Uses llama-agent default |
| - | `max_pattern_length: 100` | Uses llama-agent default |

The implementation properly handles all edge cases and provides clear logging/warnings for unsupported features.

## Acceptance Criteria Status

- [x] Configuration structure for repetition detection settings
- [x] Implementation of repetition detection algorithm (via llama-agent)  
- [x] Integration with existing LlamaAgent executor
- [x] Tests covering repetition scenarios
- [x] Documentation for configuration options