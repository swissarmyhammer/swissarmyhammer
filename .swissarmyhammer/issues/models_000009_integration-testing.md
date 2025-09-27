# Step 9: Integration Testing and Final Validation

Refer to ideas/models.md

## Objective

Complete integration testing, add comprehensive test coverage, and validate the full agent management system.

## Tasks

### 1. Add Integration Tests
- Create integration tests in `swissarmyhammer-cli/tests/`
- Test `sah agent list` with real built-in agents
- Test `sah agent use <name>` with real configuration updates
- Test error scenarios (invalid agents, permission issues)
- Test agent discovery hierarchy with mock directories

### 2. Add Unit Test Coverage
- Complete unit tests for `AgentManager` functions in config library
- Test agent precedence and overriding logic
- Test configuration file creation and updates
- Test error handling for all failure modes
- Aim for >85% test coverage on new code

### 3. Add CLI Command Tests
- Test argument parsing for agent subcommands
- Test help text and command structure
- Test output formatting for all supported formats
- Test error message clarity and usefulness

### 4. Validate Against Existing Patterns
- Ensure agent command follows same patterns as prompt/flow commands
- Verify error handling consistency with rest of CLI
- Check output formatting matches existing style
- Validate configuration integration with template system

### 5. Add End-to-End Validation
- Test complete workflow: list agents → use agent → verify config
- Test with all built-in agents (claude-code, qwen-coder, qwen-coder-flash)
- Test agent overriding with user/project agents
- Test configuration file backup and recovery

### 6. Performance and Edge Case Testing
- Test with large numbers of agents
- Test with deeply nested project structures
- Test concurrent access scenarios
- Test invalid YAML handling and recovery

## Implementation Notes

- Use existing test patterns and infrastructure
- Mock file system operations where appropriate for unit tests
- Use temporary directories for integration tests
- Add comprehensive error scenario coverage

## Acceptance Criteria

- All tests pass consistently
- Integration tests cover real-world usage scenarios
- Error handling is robust and provides helpful messages
- Performance is acceptable for reasonable agent numbers
- Agent management integrates seamlessly with existing CLI
- Documentation and help text are clear and accurate

## Files to Modify

- `swissarmyhammer-cli/tests/agent_command_tests.rs` (new)
- `swissarmyhammer-config/src/agent.rs` (add missing tests)
- Various test files for integration coverage