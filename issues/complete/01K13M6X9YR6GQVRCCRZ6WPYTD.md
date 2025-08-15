
ort logging it to noisy, set tracing by default for ort to be warn

when we do --debug all tracing should be at debug

2025-07-26T15:00:49.109152Z  INFO ort::logging: Flush-to-zero and denormal-as-zero are off

## Proposed Solution

The issue is that ONNX Runtime (used by fastembed for neural embeddings) generates very noisy logging at INFO level. The solution is to configure tracing with per-crate log level filtering:

### Implementation Plan:

1. **Replace simple `with_max_level()` with `EnvFilter`**: 
   - Use `tracing_subscriber::EnvFilter` to configure different log levels for different crates
   - Set ORT to WARN by default: `ort=warn`
   - Set all other crates to the user-requested level

2. **Update logging configuration in `swissarmyhammer-cli/src/main.rs`**:
   - When `--debug` is used: all crates including ORT get DEBUG level
   - Otherwise: ORT gets WARN level, others get the requested level (INFO by default)

3. **Implementation details**:
   - Line 81-103 in main.rs where tracing_subscriber is configured
   - Replace `.with_max_level(log_level)` with `.with_env_filter(filter)`
   - Build EnvFilter based on CLI flags

### Code Changes Required:

- `swissarmyhammer-cli/src/main.rs`: Update tracing configuration
- Test the change with semantic search operations to ensure ORT logs are quieted
