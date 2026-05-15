//! Agent command module for managing ACP server
//!
//! This module provides CLI integration for the Agent Client Protocol (ACP) server,
//! enabling swissarmyhammer to work with ACP-compatible code editors like Zed and
//! JetBrains IDEs.

pub mod acp;

use crate::context::CliContext;

/// Long description for the agent command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the agent command
pub async fn handle_command(
    subcommand: Option<crate::cli::AgentSubcommand>,
    _context: &CliContext,
) -> i32 {
    match subcommand {
        Some(crate::cli::AgentSubcommand::Acp {
            config,
            permission_policy,
            allow_path,
            block_path,
            max_file_size,
            terminal_buffer_size,
            graceful_shutdown_timeout,
        }) => {
            acp::handle_command(
                config,
                permission_policy,
                allow_path,
                block_path,
                max_file_size,
                terminal_buffer_size,
                graceful_shutdown_timeout,
            )
            .await
        }
        None => {
            eprintln!(
                "No subcommand provided. Use 'sah agent --help' to see available subcommands."
            );
            crate::exit_codes::EXIT_ERROR
        }
    }
}
