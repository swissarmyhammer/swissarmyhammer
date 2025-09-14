# Llama Model End-to-End MCP Tool Integration Test

## Summary

Create an end-to-end integration test that validates a Llama model can successfully use MCP tools through the in-process HTTP MCP server to read the Cargo.toml file. This test proves the complete integration: local model → HTTP MCP server → MCP tool → file system.

## Requirements

### Test Architecture
- Use `DEFAULT_TEST_LLM_MODEL_REPO` and `DEFAULT_TEST_LLM_MODEL_FILENAME` constants from test config
- Start in-process HTTP MCP server
- Configure Llama model to use the HTTP MCP server for tool calls
- Test the complete workflow: Model → MCP Server → file_read tool → Cargo.toml

### Test Scenario
- Initialize in-process HTTP MCP server with SwissArmyHammer tools
- Initialize Llama model with test configuration pointing to MCP server
- Provide prompt: "read the cargo.toml file using the file_read tool"
- Model should:
  1. Recognize the need to use the file_read MCP tool
  2. Make HTTP request to the in-process MCP server
  3. Call the file_read tool with correct parameters (path to Cargo.toml)
  4. Receive and process the file contents
  5. Return the contents in its response

### Technical Implementation
- Follow patterns from `swissarmyhammer/tests/llama_agent_integration.rs`
- Use in-process MCP server from `swissarmyhammer-tools/src/mcp/http_server.rs`
- Use test model constants from `swissarmyhammer-config/tests/llama_test_config.rs`
- Configure model with MCP server endpoint for tool calling
- Test should be located in `swissarmyhammer/tests/` directory

### Expected Behavior
- In-process HTTP MCP server starts successfully on random port
- Model successfully connects to MCP server for tool calls
- Model processes the prompt and identifies need for file_read tool
- Model makes correct MCP tool call with Cargo.toml path
- MCP server executes file_read tool successfully
- Model receives file contents and includes them in response
- Test validates the complete round-trip worked

### Validation Criteria
- MCP server starts and is accessible
- Model makes HTTP request to MCP server
- file_read tool is called with correct Cargo.toml path
- Response contains actual Cargo.toml content:
  - Contains `[package]` section
  - Contains `name = "swissarmyhammer"`
  - Contains dependency declarations
- Test completes within reasonable timeout (3-5 minutes max)

## Acceptance Criteria
- [ ] Test created in `swissarmyhammer/tests/llama_mcp_e2e_test.rs`
- [ ] Starts in-process HTTP MCP server with SAH tools
- [ ] Uses DEFAULT_TEST_LLM constants for model configuration
- [ ] Configures model to use MCP server for tool calls
- [ ] Provides "read the cargo.toml file using the file_read tool" prompt
- [ ] Validates model makes MCP tool call to server
- [ ] Validates file_read tool is executed correctly
- [ ] Validates response contains actual Cargo.toml content
- [ ] Test always runs (no environment variable conditions)
- [ ] Includes appropriate timeout handling for both server and model
- [ ] Follows existing code patterns and error handling
- [ ] Test passes when complete workflow succeeds
- [ ] Graceful failure handling when model lacks tool-calling capabilities

## Notes
- This is a critical end-to-end integration test proving the complete stack works
- Validates local model + in-process MCP server + SwissArmyHammer tools integration
- Demonstrates real-world usage of the model-agnostic tool architecture
- Should serve as a template for other model + MCP tool integration tests
- Proves we can use local models with our MCP tool ecosystem effectively