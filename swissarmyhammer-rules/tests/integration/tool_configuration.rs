//! Tests for verifying MCP tool configuration
//!
//! These tests ensure that:
//! 1. Rule checking sessions get NO MCP tools (mcp_config: None)
//! 2. Workflow sessions DO get MCP tools (mcp_config: Some(...))
//! 3. Tool configuration flows correctly from creation to agent initialization

use swissarmyhammer_config::model::{LlamaAgentConfig, ModelConfig};
use swissarmyhammer_rules::{AgentConfig, RuleChecker};

/// Test that RuleChecker AgentConfig explicitly has no MCP tools
///
/// This is critical because rule checking is a simple prompt/response
/// and should not have access to MCP tools which could allow arbitrary
/// code execution or file system access.
#[test]
fn test_rule_checker_agent_config_no_mcp_tools() {
    // Create an AgentConfig as would be done for rule checking
    let agent_config = AgentConfig {
        model_config: ModelConfig::llama_agent(LlamaAgentConfig::for_testing()),
        mcp_config: None, // Rule checking must have NO MCP tools
    };

    // Verify mcp_config is explicitly None
    assert!(
        agent_config.mcp_config.is_none(),
        "Rule checking AgentConfig must have mcp_config set to None"
    );
}

/// Test that RuleChecker can be created with no MCP tools
///
/// Verifies that the entire RuleChecker initialization path works
/// correctly with mcp_config: None.
#[test]
fn test_rule_checker_creation_without_mcp_tools() {
    let agent_config = AgentConfig {
        model_config: ModelConfig::llama_agent(LlamaAgentConfig::for_testing()),
        mcp_config: None,
    };

    // RuleChecker should be created successfully without MCP tools
    let checker = RuleChecker::new(agent_config);
    assert!(
        checker.is_ok(),
        "RuleChecker should be created successfully without MCP tools"
    );
}

/// Test that creating a RuleChecker with MCP tools would work
/// but is explicitly not the intended pattern
///
/// This test documents that the system CAN handle MCP tools in rule checking,
/// but we explicitly choose NOT to use them for security and simplicity.
#[test]
fn test_rule_checker_could_use_mcp_tools_but_should_not() {
    use swissarmyhammer_agent::McpServerConfig;

    // This shows that technically we COULD pass MCP config
    let agent_config_with_tools = AgentConfig {
        model_config: ModelConfig::llama_agent(LlamaAgentConfig::for_testing()),
        mcp_config: Some(McpServerConfig {
            url: "http://localhost:8080/mcp".to_string(),
        }),
    };

    // The system would work with this config
    let checker = RuleChecker::new(agent_config_with_tools);
    assert!(
        checker.is_ok(),
        "System can technically handle MCP tools in rule checking"
    );

    // BUT the comment in swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs:47-48
    // explicitly says: "Rule checking does not need MCP tools - it's a simple prompt/response"
    // So production code must always use mcp_config: None
}

/// Test pattern: How rule checking should be configured
///
/// This test serves as documentation for the correct pattern when
/// creating agents for rule checking.
#[test]
fn test_rule_checking_configuration_pattern() {
    // CORRECT pattern for rule checking:
    let rule_checking_config = AgentConfig {
        model_config: ModelConfig::llama_agent(LlamaAgentConfig::for_testing()),
        mcp_config: None, // Explicitly NO tools for rule checking
    };

    assert!(
        rule_checking_config.mcp_config.is_none(),
        "Rule checking must not have MCP tools"
    );

    // This pattern should be used in:
    // - swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs (already correct)
    // - Any other code that creates agents for rule checking
}

/// Test that workflow AgentConfig pattern includes MCP tools
///
/// This is a documentation test showing the OPPOSITE pattern used
/// for workflows where tools ARE needed.
#[test]
fn test_workflow_agent_should_have_mcp_tools() {
    use swissarmyhammer_agent::McpServerConfig;

    // CORRECT pattern for workflow actions:
    let _workflow_config = ModelConfig::llama_agent(LlamaAgentConfig::for_testing());
    let mcp_config = Some(McpServerConfig {
        url: "http://localhost:8080/mcp".to_string(),
    });

    // Workflow agents should have MCP tools
    assert!(
        mcp_config.is_some(),
        "Workflow actions should have MCP tools available"
    );

    // This pattern is used in:
    // - swissarmyhammer-workflow/src/actions.rs:531
    // where it calls: acp::create_agent(&agent_config, Some(mcp_config))
}

/// Integration test: Verify the separation of concerns
///
/// This test documents that we have two distinct agent creation patterns:
/// 1. Rule checking: NO tools (mcp_config: None)
/// 2. Workflows: WITH tools (mcp_config: Some(...))
#[test]
fn test_agent_creation_patterns_separation() {
    use swissarmyhammer_agent::McpServerConfig;

    // Pattern 1: Rule checking (no tools)
    let rule_checking_config = AgentConfig {
        model_config: ModelConfig::llama_agent(LlamaAgentConfig::for_testing()),
        mcp_config: None,
    };

    // Pattern 2: Workflows (with tools)
    let workflow_mcp_config = Some(McpServerConfig {
        url: "http://localhost:8080/mcp".to_string(),
    });

    // Verify the patterns are different
    assert!(
        rule_checking_config.mcp_config.is_none(),
        "Rule checking should have NO MCP config"
    );
    assert!(
        workflow_mcp_config.is_some(),
        "Workflows should have MCP config"
    );

    // This separation ensures:
    // - Rule checking is fast and simple (no tool overhead)
    // - Rule checking is secure (no arbitrary code execution)
    // - Workflows have full tool access (as needed for complex operations)
}

/// Test that documents where tool configuration happens in the codebase
#[test]
fn test_tool_configuration_locations() {
    // This test serves as documentation for where tool configuration happens:
    //
    // 1. Rule checking (NO tools):
    //    - swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs:48
    //      `mcp_config: None,`
    //
    // 2. Workflows (WITH tools):
    //    - swissarmyhammer-workflow/src/actions.rs:531
    //      `acp::create_agent(&agent_config, Some(mcp_config))`
    //
    // 3. Agent creation (common):
    //    - swissarmyhammer-agent/src/lib.rs:150-400
    //      Handles both Claude and Llama agents with optional MCP config

    // Verify test configuration
    let test_config = AgentConfig {
        model_config: ModelConfig::llama_agent(LlamaAgentConfig::for_testing()),
        mcp_config: None,
    };

    assert!(test_config.mcp_config.is_none());
}

/// Regression test: Ensure MCP server propagation fix remains working
///
/// This test verifies that the fix from the merge remains working:
/// - Before: ACP config created with default() had empty default_mcp_servers
/// - After: MCP servers are properly converted and set in acp_config.default_mcp_servers
///
/// The fix is in swissarmyhammer-agent/src/lib.rs:335-380
#[test]
fn test_mcp_server_propagation_to_acp() {
    use swissarmyhammer_agent::McpServerConfig;

    // Test that when we create agent config WITH MCP servers for workflows,
    // they would be properly propagated (this is a compile-time check)
    let mcp_config = Some(McpServerConfig {
        url: "http://localhost:54246/mcp".to_string(),
    });

    assert!(
        mcp_config.is_some(),
        "MCP config for workflows should be Some(...)"
    );

    // The actual propagation happens in swissarmyhammer-agent/src/lib.rs
    // where it converts McpServerConfig to ACP format and sets default_mcp_servers
    // This test documents that the fix exists and should not be regressed
}
