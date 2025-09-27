# Step 10: Documentation, CLI Help, and Final Cleanup

Refer to ideas/models.md

## Objective

Complete the agent management implementation with proper documentation, help text, and final cleanup.

## Tasks

### 1. Add CLI Help Documentation
- Update main CLI help to include agent command description
- Add detailed help text for `sah agent` command
- Add help text for `sah agent list` and `sah agent use` subcommands
- Follow existing help text patterns and formatting

### 2. Add Usage Examples
- Add practical examples to help text
- Show common workflows: list → use → verify
- Include examples of agent overriding scenarios
- Document built-in agent names and purposes

### 3. Update Build and CI Integration
- Ensure new build steps work in CI environment
- Update any build documentation if needed
- Verify embedded agents are included in release builds
- Test build process on all supported platforms

### 4. Add Library Documentation
- Document public API functions in `swissarmyhammer-config`
- Add module-level documentation for agent management
- Update crate-level documentation with agent features
- Add usage examples in doc comments

### 5. Error Message Refinement
- Review all error messages for clarity and helpfulness
- Ensure error messages guide users toward solutions
- Add suggestions for common mistakes
- Standardize error format with existing CLI commands

### 6. Final Validation and Cleanup
- Run full test suite and fix any issues
- Clean up temporary files and debug code
- Remove draft plan file from `.swissarmyhammer/tmp/`
- Verify no broken references or unused imports

## Implementation Notes

- Use existing documentation patterns from prompt/flow commands
- Keep help text concise but comprehensive
- Focus on practical usage scenarios in examples
- Ensure all public APIs are properly documented

## Acceptance Criteria

- Help text is clear, accurate, and follows existing patterns
- All public APIs have appropriate documentation
- Build process works correctly in all environments
- Error messages provide actionable guidance
- No temporary or debug artifacts remain in final code
- Full functionality is ready for production use

## Files to Modify

- `swissarmyhammer-cli/src/cli.rs` (help text)
- `swissarmyhammer-config/src/agent.rs` (documentation)
- `swissarmyhammer-config/src/lib.rs` (module docs)
- Various files for cleanup and final polish