make sure that all errors containing ABORT ERROR really do abort, meaning lot that one last error and hard exit with a non zero exit code. setup up a test with the abort.md prompt to test that a response from the model with ABORT ERROR really does abort, meaning hard exit with a non zero

## Proposed Solution

After analyzing the codebase, I found that ABORT ERROR handling exists in several places:

1. **Current ABORT ERROR Detection**: 
   - `swissarmyhammer-cli/src/error.rs:34` - `is_abort_error()` function detects "ABORT ERROR" in error messages
   - `swissarmyhammer-cli/src/main.rs:274` - Main function checks for "ABORT ERROR" and sets `EXIT_ERROR` (exit code 2)
   - `swissarmyhammer/src/common/abort_handler.rs` - Comprehensive abort error detection and handling

2. **Test Infrastructure Needed**:
   - Create a CLI integration test that runs the `abort.md` prompt
   - Verify that when the prompt response contains "ABORT ERROR", the CLI exits with code 2
   - Ensure the test captures both stdout/stderr and the process exit code

3. **Implementation Steps**:
   1. Create test in `swissarmyhammer-cli/tests/` that runs `prompt test abort` command
   2. Verify the command exits with code 2 (EXIT_ERROR) when ABORT ERROR is detected
   3. Ensure all existing ABORT ERROR code paths consistently use EXIT_ERROR (code 2)
   4. Test the complete flow: prompt execution → ABORT ERROR detection → process termination

The existing infrastructure appears solid, but we need to verify end-to-end behavior with a proper integration test.