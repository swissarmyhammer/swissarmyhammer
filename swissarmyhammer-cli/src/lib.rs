// Re-export modules for use in tests and integration
pub mod cli;
pub mod cli_builder;
pub mod cli_optimization;
pub mod completions;
pub mod debug;
pub mod dynamic_execution;
pub mod error;
pub mod exit_codes;
pub mod mcp_integration;
pub mod parameter_cli;
pub mod response_formatting;
pub mod schema_conversion;
pub mod validate;

// Static command modules (preserved)
pub mod config;
pub mod doctor;
pub mod flow;
pub mod list;
pub mod logging;
pub mod prompt;
pub mod search;
pub mod test;

// Re-export key types for easy access
pub use cli_builder::CliBuilder;
pub use dynamic_execution::DynamicCommandExecutor;
pub use debug::CliDebugger;
pub use cli_optimization::{get_or_build_cli, handle_fast_path_commands};
pub use schema_conversion::SchemaConverter;
