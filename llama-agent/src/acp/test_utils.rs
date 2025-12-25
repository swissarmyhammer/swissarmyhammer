//! Test utilities for ACP server creation
//!
//! Helper functions for creating ACP servers in tests with minimal configuration.

use crate::acp::{AcpConfig, AcpServer};
use crate::types::AgentConfig;
use std::sync::Arc;

/// Create ACP server for testing with custom config
pub async fn create_acp_server(
    config: AgentConfig,
) -> Result<
    (
        AcpServer,
        tokio::sync::broadcast::Receiver<agent_client_protocol::SessionNotification>,
    ),
    Box<dyn std::error::Error>,
> {
    let model_manager = Arc::new(crate::model::ModelManager::new(config.model.clone())?);

    // Load model
    model_manager.load_model().await?;

    let request_queue = Arc::new(crate::queue::RequestQueue::new(
        model_manager.clone(),
        config.queue_config.clone(),
        config.session_config.clone(),
    ));
    let session_manager = Arc::new(crate::session::SessionManager::new(config.session_config.clone()));
    let mcp_client: Arc<dyn crate::mcp::MCPClient> = Arc::new(crate::mcp::NoOpMCPClient::new());
    let chat_template = Arc::new(crate::chat_template::ChatTemplateEngine::new());
    let dependency_analyzer = Arc::new(crate::dependency_analysis::DependencyAnalyzer::new(
        config.parallel_execution_config.clone(),
    ));

    let agent_server = Arc::new(crate::AgentServer::new(
        model_manager,
        request_queue,
        session_manager,
        mcp_client,
        chat_template,
        dependency_analyzer,
        config,
    ));

    let acp_config = AcpConfig::default();
    Ok(AcpServer::new(agent_server, acp_config))
}
