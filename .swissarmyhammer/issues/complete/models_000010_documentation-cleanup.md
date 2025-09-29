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
## Proposed Solution

After examining the codebase, I've identified the current state and developed a comprehensive approach for completing the documentation and cleanup phase:

### Current State Analysis
- CLI help structure is already well-established with consistent patterns
- Agent system is fully implemented with basic help text in place
- Agent configuration types are comprehensively documented in the library code
- Main gap is in enhanced help text, API documentation, and final cleanup

### Implementation Plan

#### 1. CLI Help Documentation Enhancement
- **Main CLI Help** (`swissarmyhammer-cli/src/cli.rs:line_92`): Update the main command's `long_about` to include comprehensive agent command description
- **Agent Command Help** (`swissarmyhammer-cli/src/commands/agent/description.md`): Enhance existing basic help with detailed usage scenarios, workflow examples, and built-in agent descriptions
- **Subcommand Help**: Add detailed `long_about` sections for `sah agent list` and `sah agent use` following existing patterns

#### 2. Usage Examples Integration
- Add practical workflow examples showing: list → use → verify
- Document common agent overriding scenarios (built-in → project → user precedence)
- Include examples of all built-in agents: `claude-code`, `qwen-coder` 
- Show error handling and troubleshooting scenarios

#### 3. Library API Documentation
- **Public Functions** (`swissarmyhammer-config/src/agent.rs`): Add comprehensive doc comments for all public functions in `AgentManager`
- **Module Documentation**: Enhance module-level docs with usage examples and architectural overview
- **Error Documentation**: Improve error type documentation with common causes and solutions

#### 4. Error Message Refinement
- Review all error messages in agent commands for clarity and actionability
- Ensure error messages guide users toward solutions
- Standardize error format with existing CLI commands

#### 5. Final Cleanup and Validation
- Remove any temporary or debug artifacts
- Run comprehensive test suite and fix issues
- Verify build process works correctly across platforms
- Clean up any remaining draft files or unused imports

### Files to Modify
1. `swissarmyhammer-cli/src/cli.rs` - Main CLI help text
2. `swissarmyhammer-cli/src/commands/agent/description.md` - Enhanced agent help
3. `swissarmyhammer-config/src/agent.rs` - API documentation 
4. `swissarmyhammer-config/src/lib.rs` - Module documentation
5. Various command files for error message refinement