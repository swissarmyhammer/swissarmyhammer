//! Fixture support for llama-agent ACP server

use agent_client_protocol_extras::{AgentWithFixture, FixtureMode};
use std::sync::Arc;

impl AgentWithFixture for super::AcpServer {
    fn agent_type(&self) -> &'static str {
        "llama"
    }

    fn with_fixture(&mut self, test_name: &str) {
        let mode = self.fixture_mode(test_name);

        // Get mutable access to the underlying AgentServer config
        // SAFETY: We're in test context, single-threaded access
        let agent_server_ptr = Arc::as_ptr(&self.agent_server) as *mut crate::AgentServer;
        unsafe {
            let agent_server = &mut *agent_server_ptr;

            agent_server.config.mode = match mode {
                FixtureMode::Record { path } => {
                    tracing::info!("Configuring llama-agent for record mode: {:?}", path);
                    crate::types::LlamaAgentMode::Record { output_path: path }
                }
                FixtureMode::Playback { path } => {
                    tracing::info!("Configuring llama-agent for playback mode: {:?}", path);
                    crate::types::LlamaAgentMode::Playback { input_path: path }
                }
                FixtureMode::Normal => {
                    tracing::info!("Configuring llama-agent for normal mode");
                    crate::types::LlamaAgentMode::Normal
                }
            };

            // Recreate generation backend with new mode
            agent_server.generation_backend = match &agent_server.config.mode {
                crate::types::LlamaAgentMode::Normal => {
                    Arc::new(crate::generation_backend::RealGenerationBackend::new(
                        agent_server.request_queue.clone(),
                        agent_server.session_manager.clone(),
                    ))
                }
                crate::types::LlamaAgentMode::Playback { input_path } => {
                    match crate::generation_backend::RecordedGenerationBackend::from_file(
                        input_path,
                    ) {
                        Ok(backend) => Arc::new(backend),
                        Err(_) => Arc::new(crate::generation_backend::RealGenerationBackend::new(
                            agent_server.request_queue.clone(),
                            agent_server.session_manager.clone(),
                        )),
                    }
                }
                crate::types::LlamaAgentMode::Record { output_path } => {
                    Arc::new(crate::generation_backend::RecordingGenerationBackend::new(
                        Arc::new(crate::generation_backend::RealGenerationBackend::new(
                            agent_server.request_queue.clone(),
                            agent_server.session_manager.clone(),
                        )),
                        output_path.clone(),
                    ))
                }
            };
        }
    }
}
