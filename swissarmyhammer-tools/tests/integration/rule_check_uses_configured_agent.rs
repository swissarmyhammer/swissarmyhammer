//! Test that proves ToolContext correctly resolves configured agents for different use cases

use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_config::model::{LlamaAgentConfig, LlmModelConfig, ModelSource};
use swissarmyhammer_config::{AgentUseCase, ModelConfig};
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::ToolContext;
use tokio::sync::Mutex as TokioMutex;

/// Create a test context with a specific agent for the Rules use case
async fn create_test_context_with_agent(rules_agent: ModelConfig) -> ToolContext {
    let git_ops: Arc<TokioMutex<Option<GitOperations>>> = Arc::new(TokioMutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());

    // Root agent is default
    let root_agent = Arc::new(ModelConfig::default());

    // Create use case agents map with our test agent for Rules
    let mut use_case_agents = HashMap::new();
    use_case_agents.insert(AgentUseCase::Rules, Arc::new(rules_agent));

    let mut context = ToolContext::new(tool_handlers, git_ops, root_agent);
    context.use_case_agents = Arc::new(use_case_agents);
    context
}

#[tokio::test]
async fn test_tool_context_model_resolution() {
    // Test that ToolContext correctly returns different agents for different use cases

    // Create qwen config
    let qwen_config = ModelConfig {
        executor: swissarmyhammer_config::model::ModelExecutorConfig::LlamaAgent(
            LlamaAgentConfig {
                model: LlmModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: "test/repo".to_string(),
                        filename: Some("test.gguf".to_string()),
                        folder: None,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
        quiet: false,
    };

    let context = create_test_context_with_agent(qwen_config).await;

    // Get agent for Rules use case
    let rules_agent = context.get_agent_for_use_case(AgentUseCase::Rules);

    eprintln!("\n=== ToolContext Agent Resolution ===");
    eprintln!(
        "Rules agent executor type: {:?}",
        rules_agent.executor_type()
    );

    // Should be LlamaAgent (qwen)
    assert!(
        matches!(
            rules_agent.executor_type(),
            swissarmyhammer_config::model::ModelExecutorType::LlamaAgent
        ),
        "Rules should use LlamaAgent"
    );

    // Root should fall back to default (ClaudeCode)
    let root_agent = context.get_agent_for_use_case(AgentUseCase::Root);
    eprintln!("Root agent executor type: {:?}", root_agent.executor_type());

    assert!(
        matches!(
            root_agent.executor_type(),
            swissarmyhammer_config::model::ModelExecutorType::ClaudeCode
        ),
        "Root should use ClaudeCode (default)"
    );

    eprintln!("✓ ToolContext correctly returns different agents for different use cases");
}
