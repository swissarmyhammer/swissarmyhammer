//! Available commands handling for Agent Client Protocol
//!
//! This module manages the discovery and notification of available commands
//! for sessions, including integration with MCP servers and tool handlers.

use agent_client_protocol::{SessionId, SessionNotification, SessionUpdate};

impl crate::agent::ClaudeAgent {
    /// Send available commands update notification
    ///
    /// Sends available commands update via SessionUpdate::AvailableCommandsUpdate
    /// when command availability changes during session execution.
    pub async fn send_available_commands_update(
        &self,
        session_id: &SessionId,
        commands: Vec<agent_client_protocol::AvailableCommand>,
    ) -> crate::Result<()> {
        let update = SessionUpdate::AvailableCommandsUpdate(
            agent_client_protocol::AvailableCommandsUpdate::new(commands),
        );

        // Store in session context for history replay
        let commands_message = crate::session::Message::from_update(update.clone());

        // Convert ACP SessionId to internal SessionId
        let internal_session_id = crate::session::SessionId::parse(&session_id.to_string())
            .map_err(|e| crate::AgentError::Protocol(format!("Invalid session ID: {}", e)))?;

        self.session_manager
            .update_session(&internal_session_id, |session| {
                session.add_message(commands_message);
            })
            .map_err(|e| {
                tracing::error!("Failed to update session: {}", e);
                crate::AgentError::Protocol("Failed to update session".to_string())
            })?;

        let mut meta = serde_json::Map::new();
        meta.insert(
            "update_type".to_string(),
            serde_json::json!("available_commands"),
        );
        meta.insert(
            "session_id".to_string(),
            serde_json::json!(session_id.to_string()),
        );
        meta.insert(
            "timestamp".to_string(),
            serde_json::json!(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()),
        );

        let notification = SessionNotification::new(session_id.clone(), update).meta(meta);

        tracing::debug!(
            "Sending available commands update for session: {}",
            session_id
        );
        self.send_session_update(notification).await
    }

    /// Update available commands for a session and send notification if changed
    ///
    /// This method updates the session's available commands and sends an
    /// AvailableCommandsUpdate notification if the commands have changed.
    /// Returns true if an update was sent, false if commands were unchanged.
    pub async fn update_session_available_commands(
        &self,
        session_id: &SessionId,
        commands: Vec<agent_client_protocol::AvailableCommand>,
    ) -> crate::Result<bool> {
        // Parse SessionId from ACP format (raw ULID)
        let parsed_session_id = crate::session::SessionId::parse(&session_id.0)
            .map_err(|e| crate::AgentError::Session(format!("Invalid session ID format: {}", e)))?;

        // Update commands in session manager
        let commands_changed = self
            .session_manager
            .update_available_commands(&parsed_session_id, commands.clone())?;

        // Send notification if commands changed
        if commands_changed {
            self.send_available_commands_update(session_id, commands.clone())
                .await?;
            tracing::info!(
                "Sent available commands update for session: {} ({} commands)",
                session_id,
                commands.len()
            );
        }

        Ok(commands_changed)
    }

    /// Refresh available commands for all active sessions
    ///
    /// This method is called when MCP servers send notifications about capability changes
    /// (tools/list_changed or prompts/list_changed). It updates commands for all active
    /// sessions and sends AvailableCommandsUpdate notifications if commands have changed.
    pub async fn refresh_commands_for_all_sessions(&self) {
        tracing::debug!("Refreshing available commands for all active sessions");

        // Get list of all active sessions
        let session_ids = match self.session_manager.list_sessions() {
            Ok(ids) => ids,
            Err(e) => {
                tracing::error!("Failed to list sessions for command refresh: {}", e);
                return;
            }
        };

        // Refresh commands for each session
        for session_id in session_ids {
            let protocol_session_id = SessionId::new(session_id.to_string());

            // Get updated commands for this session
            let updated_commands = self
                .get_available_commands_for_session(&protocol_session_id)
                .await;

            // Update and notify if changed
            if let Err(e) = self
                .update_session_available_commands(&protocol_session_id, updated_commands)
                .await
            {
                tracing::warn!(
                    "Failed to update commands for session {}: {}",
                    session_id,
                    e
                );
            }
        }

        tracing::debug!("Completed command refresh for all active sessions");
    }

    /// Get available commands for a session
    ///
    /// This method determines what commands are available for the given session
    /// based on capabilities, MCP servers, and current session state.
    /// Get available commands for a session, filtered by client capabilities
    ///
    /// This method returns the list of commands (slash commands) available to the client
    /// for the given session. The returned commands are automatically filtered based on
    /// the client's declared capabilities during initialization.
    ///
    /// # Capability Filtering
    ///
    /// Commands are only included if the client has declared the necessary capabilities:
    /// - Core planning and analysis commands are always available
    /// - MCP-provided commands are included based on connected MCP servers
    /// - Tool-based commands respect the client's declared tool capabilities
    ///
    /// This ensures that operations requiring specific capabilities (like file system
    /// or terminal access) are only offered to clients that support them, maintaining
    /// the ACP contract that all operations must check capabilities before execution.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier to get commands for
    ///
    /// # Returns
    ///
    /// A vector of AvailableCommand structs representing commands the client can invoke
    pub(crate) async fn get_available_commands_for_session(
        &self,
        session_id: &SessionId,
    ) -> Vec<agent_client_protocol::AvailableCommand> {
        let mut commands = Vec::new();

        // Always available core commands
        let mut meta1 = serde_json::Map::new();
        meta1.insert("category".to_string(), serde_json::json!("planning"));
        meta1.insert("source".to_string(), serde_json::json!("core"));

        commands.push(
            agent_client_protocol::AvailableCommand::new(
                "create_plan".to_string(),
                "Create an execution plan for complex tasks".to_string(),
            )
            .meta(meta1),
        );

        let mut meta2 = serde_json::Map::new();
        meta2.insert("category".to_string(), serde_json::json!("analysis"));
        meta2.insert("source".to_string(), serde_json::json!("core"));

        commands.push(
            agent_client_protocol::AvailableCommand::new(
                "research_codebase".to_string(),
                "Research and analyze the codebase structure".to_string(),
            )
            .meta(meta2),
        );

        // Add commands from MCP servers - use prompts, not tools
        if let Some(mcp_manager) = &self.mcp_manager {
            let mcp_prompts = mcp_manager.list_available_prompts().await;
            tracing::debug!(
                "MCP manager returned {} prompts for slash commands",
                mcp_prompts.len()
            );

            for prompt in mcp_prompts {
                tracing::debug!(
                    "Adding MCP prompt as slash command: {} - {}",
                    prompt.name,
                    prompt.description.as_deref().unwrap_or("(no description)")
                );
                let input_hint = if prompt.arguments.is_empty() {
                    None
                } else {
                    Some(
                        prompt
                            .arguments
                            .iter()
                            .map(|arg| {
                                if arg.required {
                                    format!("<{}>", arg.name)
                                } else {
                                    format!("[{}]", arg.name)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                    )
                };

                let description_with_hint = if let Some(hint) = input_hint.as_ref() {
                    format!(
                        "{} {}",
                        prompt
                            .description
                            .clone()
                            .unwrap_or_else(|| format!("MCP prompt: {}", prompt.name)),
                        hint
                    )
                } else {
                    prompt
                        .description
                        .clone()
                        .unwrap_or_else(|| format!("MCP prompt: {}", prompt.name))
                };

                // Build parameter schema for meta field
                let parameters_schema: Vec<serde_json::Value> = prompt
                    .arguments
                    .iter()
                    .map(|arg| {
                        serde_json::json!({
                            "name": arg.name,
                            "description": arg.description,
                            "required": arg.required,
                        })
                    })
                    .collect();

                // Create input specification if there are arguments
                let command_input = if let Some(hint) = input_hint {
                    let mut input_meta = serde_json::Map::new();
                    input_meta.insert(
                        "parameters".to_string(),
                        serde_json::Value::Array(parameters_schema.clone()),
                    );

                    Some(agent_client_protocol::AvailableCommandInput::Unstructured(
                        agent_client_protocol::UnstructuredCommandInput::new(hint).meta(input_meta),
                    ))
                } else {
                    None
                };

                let mut meta = serde_json::Map::new();
                meta.insert("category".to_string(), serde_json::json!("mcp_prompt"));
                meta.insert("source".to_string(), serde_json::json!("mcp_server"));
                meta.insert(
                    "arguments".to_string(),
                    serde_json::json!(parameters_schema),
                );

                let mut cmd = agent_client_protocol::AvailableCommand::new(
                    prompt.name.clone(),
                    description_with_hint,
                )
                .meta(meta);

                if let Some(input) = command_input {
                    cmd = cmd.input(input);
                }

                commands.push(cmd);
            }
        }

        // Add commands from tool handler based on capabilities
        let tool_handler = self.tool_handler.read().await;
        let tool_names = tool_handler.list_all_available_tools().await;
        drop(tool_handler);

        // Get client capabilities to filter tools
        let client_caps = self.client_capabilities.read().await;
        let has_fs_read = client_caps
            .as_ref()
            .is_some_and(|caps| caps.fs.read_text_file);
        let has_fs_write = client_caps
            .as_ref()
            .is_some_and(|caps| caps.fs.write_text_file);
        let has_terminal_capability = client_caps.as_ref().is_some_and(|caps| caps.terminal);
        drop(client_caps);

        for tool_name in tool_names {
            // Filter based on capabilities
            let should_include = match tool_name.as_str() {
                "fs_read" | "fs_list" => has_fs_read,
                "fs_write" => has_fs_write,
                name if name.starts_with("terminal_") => has_terminal_capability,
                _ => true, // Include other tools by default
            };

            if should_include {
                let (category, description) = match tool_name.as_str() {
                    "fs_read" => ("filesystem", "Read file contents"),
                    "fs_write" => ("filesystem", "Write file contents"),
                    "fs_list" => ("filesystem", "List directory contents"),
                    "terminal_create" => ("terminal", "Create a new terminal session"),
                    "terminal_write" => ("terminal", "Write to a terminal session"),
                    _ => ("tool", "Tool handler command"),
                };

                let mut meta = serde_json::Map::new();
                meta.insert("category".to_string(), serde_json::json!(category));
                meta.insert("source".to_string(), serde_json::json!("tool_handler"));

                commands.push(
                    agent_client_protocol::AvailableCommand::new(
                        tool_name.clone(),
                        description.to_string(),
                    )
                    .meta(meta),
                );
            }
        }

        tracing::debug!(
            "Generated {} available commands for session {} (mcp: {}, tool_handler: {})",
            commands.len(),
            session_id,
            if self.mcp_manager.is_some() {
                commands
                    .iter()
                    .filter(|c| {
                        c.meta
                            .as_ref()
                            .and_then(|m| m.get("source"))
                            .and_then(|s| s.as_str())
                            == Some("mcp_server")
                    })
                    .count()
            } else {
                0
            },
            commands
                .iter()
                .filter(|c| c
                    .meta
                    .as_ref()
                    .and_then(|m| m.get("source"))
                    .and_then(|s| s.as_str())
                    == Some("tool_handler"))
                .count()
        );

        tracing::debug!(
            "Total available commands for session {}: {}",
            session_id,
            commands.len()
        );
        tracing::debug!(
            "Command names: {:?}",
            commands.iter().map(|c| &c.name).collect::<Vec<_>>()
        );

        commands
    }
}
