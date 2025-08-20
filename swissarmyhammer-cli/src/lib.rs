// Re-export modules for use in tests
pub mod cli;
pub mod cli_builder;
pub mod dynamic_execution;
pub mod error;
pub mod exit_codes;
pub mod mcp_integration;
pub mod parameter_cli;
pub mod response_formatting;
pub mod schema_conversion;
pub mod validate;

// Re-export CliBuilder for easy access
pub use cli_builder::CliBuilder;
