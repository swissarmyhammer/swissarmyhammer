//! Test helpers for ACP server

use super::AcpServer;
use crate::types::AgentConfig;
use std::sync::Arc;

impl AcpServer {
    /// Create AcpServer for testing with default configuration
    pub fn for_testing(agent_config: Option<AgentConfig>) -> Result<Self, Box<dyn std::error::Error>> {
        let config = agent_config.unwrap_or_default();

        let model_manager = Arc::new(crate::model::ModelManager::new(config.model.clone())?);
        let request_queue = Arc::new(crate::queue::RequestQueue::new(
            model_manager.clone(),
            config.queue_config.clone(),
            config.session_config.clone(),
        ));
        let session_manager = Arc::new(crate::session::SessionManager::new(
            config.session_config.clone(),
        ));
        let mcp_client: Arc<dyn crate::mcp::MCPClient> =
            Arc::new(crate::mcp::NoOpMCPClient::new());
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

        let acp_config = super::AcpConfig::default();
        Ok(Self::new(agent_server, acp_config))
    }
}
