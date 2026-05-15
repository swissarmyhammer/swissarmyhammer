//! Terminal operation handlers for ClaudeAgent
//!
//! This module contains methods for managing terminal operations within agent sessions,
//! including terminal creation, output retrieval, release, wait for exit, and kill operations.

use crate::ClaudeAgent;
use swissarmyhammer_common::Pretty;

impl ClaudeAgent {
    /// Handle terminal/output ACP extension method
    ///
    /// Retrieves output from a running terminal session.
    ///
    /// # Arguments
    ///
    /// * `params` - Terminal output request parameters
    ///
    /// # Returns
    ///
    /// Returns terminal output response containing the output data
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Terminal capability is not declared by client
    /// - Client capabilities are not initialized
    /// - Terminal output retrieval fails
    pub async fn handle_terminal_output(
        &self,
        params: crate::terminal_manager::TerminalOutputParams,
    ) -> Result<crate::terminal_manager::TerminalOutputResponse, agent_client_protocol::Error> {
        tracing::debug!("Processing terminal/output request: {}", Pretty(&params));

        // Check client terminal capability before allowing operation
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.terminal => {
                    tracing::debug!("Terminal capability validated for handle_terminal_output");
                }
                Some(_) => {
                    tracing::error!("terminal capability not declared by client");
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Terminal capability not declared by client. Set client_capabilities.terminal = true during initialization.".to_string(),
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for terminal operation validation"
                    );
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Client capabilities not initialized. Cannot perform terminal operations without capability declaration.".to_string(),
                    ));
                }
            }
        }

        // Get terminal manager from tool handler
        let tool_handler = self.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        // Get output from terminal manager
        terminal_manager
            .get_output(&self.session_manager, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get terminal output: {}", e);
                e.into()
            })
    }

    /// Handle terminal/release ACP extension method
    ///
    /// Releases a terminal session, allowing it to be garbage collected.
    ///
    /// # Arguments
    ///
    /// * `params` - Terminal release request parameters
    ///
    /// # Returns
    ///
    /// Returns null per ACP specification
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Terminal capability is not declared by client
    /// - Client capabilities are not initialized
    /// - Terminal release fails
    pub async fn handle_terminal_release(
        &self,
        params: crate::terminal_manager::TerminalReleaseParams,
    ) -> Result<serde_json::Value, agent_client_protocol::Error> {
        tracing::debug!("Processing terminal/release request: {}", Pretty(&params));

        // Check client terminal capability before allowing operation
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.terminal => {
                    tracing::debug!("Terminal capability validated for handle_terminal_release");
                }
                Some(_) => {
                    tracing::error!("terminal capability not declared by client");
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Terminal capability not declared by client. Set client_capabilities.terminal = true during initialization.".to_string(),
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for terminal operation validation"
                    );
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Client capabilities not initialized. Cannot perform terminal operations without capability declaration.".to_string(),
                    ));
                }
            }
        }

        // Get terminal manager from tool handler
        let tool_handler = self.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        // Release terminal and return null per ACP specification
        terminal_manager
            .release_terminal(&self.session_manager, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to release terminal: {}", e);
                e.into()
            })
    }

    /// Handle terminal/wait_for_exit ACP extension method
    ///
    /// Waits for a terminal process to exit and returns its exit status.
    ///
    /// # Arguments
    ///
    /// * `params` - Terminal wait for exit request parameters
    ///
    /// # Returns
    ///
    /// Returns the exit status of the terminal process
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Terminal capability is not declared by client
    /// - Client capabilities are not initialized
    /// - Wait for exit operation fails
    pub async fn handle_terminal_wait_for_exit(
        &self,
        params: crate::terminal_manager::TerminalOutputParams,
    ) -> Result<crate::terminal_manager::ExitStatus, agent_client_protocol::Error> {
        tracing::debug!(
            "Processing terminal/wait_for_exit request: {}",
            Pretty(&params)
        );

        // Check client terminal capability before allowing operation
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.terminal => {
                    tracing::debug!(
                        "Terminal capability validated for handle_terminal_wait_for_exit"
                    );
                }
                Some(_) => {
                    tracing::error!("terminal capability not declared by client");
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Terminal capability not declared by client. Set client_capabilities.terminal = true during initialization.".to_string(),
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for terminal operation validation"
                    );
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Client capabilities not initialized. Cannot perform terminal operations without capability declaration.".to_string(),
                    ));
                }
            }
        }

        // Get terminal manager from tool handler
        let tool_handler = self.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        // Wait for terminal exit
        terminal_manager
            .wait_for_exit(&self.session_manager, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to wait for terminal exit: {}", e);
                e.into()
            })
    }

    /// Handle terminal/kill ACP extension method
    ///
    /// Forcefully terminates a running terminal process.
    ///
    /// # Arguments
    ///
    /// * `params` - Terminal kill request parameters
    ///
    /// # Returns
    ///
    /// Returns unit on successful termination
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Terminal capability is not declared by client
    /// - Client capabilities are not initialized
    /// - Terminal kill operation fails
    pub async fn handle_terminal_kill(
        &self,
        params: crate::terminal_manager::TerminalOutputParams,
    ) -> Result<(), agent_client_protocol::Error> {
        tracing::debug!("Processing terminal/kill request: {}", Pretty(&params));

        // Check client terminal capability before allowing operation
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.terminal => {
                    tracing::debug!("Terminal capability validated for handle_terminal_kill");
                }
                Some(_) => {
                    tracing::error!("terminal capability not declared by client");
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Terminal capability not declared by client. Set client_capabilities.terminal = true during initialization.".to_string(),
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for terminal operation validation"
                    );
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Client capabilities not initialized. Cannot perform terminal operations without capability declaration.".to_string(),
                    ));
                }
            }
        }

        // Get terminal manager from tool handler
        let tool_handler = self.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        // Kill terminal process
        terminal_manager
            .kill_terminal(&self.session_manager, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to kill terminal: {}", e);
                e.into()
            })
    }

    /// Handle terminal/create ACP extension method
    ///
    /// Creates a new terminal session with a command.
    ///
    /// # Arguments
    ///
    /// * `params` - Terminal creation request parameters
    ///
    /// # Returns
    ///
    /// Returns the terminal ID of the newly created terminal
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Terminal capability is not declared by client
    /// - Client capabilities are not initialized
    /// - Terminal creation fails
    pub async fn handle_terminal_create(
        &self,
        params: crate::terminal_manager::TerminalCreateParams,
    ) -> Result<crate::terminal_manager::TerminalCreateResponse, agent_client_protocol::Error> {
        tracing::debug!("Processing terminal/create request: {}", Pretty(&params));

        // Check client terminal capability before allowing operation
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.terminal => {
                    tracing::debug!("Terminal capability validated for handle_terminal_create");
                }
                Some(_) => {
                    tracing::error!("terminal capability not declared by client");
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Terminal capability not declared by client. Set client_capabilities.terminal = true during initialization.".to_string(),
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for terminal operation validation"
                    );
                    return Err(agent_client_protocol::Error::new(
                        -32602,
                        "Client capabilities not initialized. Cannot perform terminal operations without capability declaration.".to_string(),
                    ));
                }
            }
        }

        // Get terminal manager from tool handler
        let tool_handler = self.tool_handler.read().await;
        let terminal_manager = tool_handler.get_terminal_manager();

        // Create terminal and return the terminal ID
        let terminal_id = terminal_manager
            .create_terminal_with_command(&self.session_manager, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create terminal: {}", e);
                agent_client_protocol::Error::from(e)
            })?;

        Ok(crate::terminal_manager::TerminalCreateResponse { terminal_id })
    }
}
