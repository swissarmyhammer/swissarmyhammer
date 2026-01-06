//! Integration test modules for swissarmyhammer-cli

// Note: abort_comprehensives and abort_final_integrations tests were removed
// as they tested the old file-based abort system which has been migrated to CEL state.
mod agent_command;
mod binary_aliases;
mod builtin_validation;
mod cli_integration;
mod cli_mcp_integration;
mod cli_serve_http;
mod comprehensive_cli_mcp_integrations;
mod doc_examples;
mod e2e_workflows;
mod error_scenarios;
mod example_actions_workflow;
mod git_repository_error_handlings;
mod hello_world_workflow;
mod mcp_e2es;
mod mcp_integration;
mod mcp_tools_registration;
mod model_cli_parsings;
mod model_commands;
mod model_e2e_workflows;
mod model_list_units;
mod model_performance_edge_casess;
mod model_use_case_integration;
mod prompt_command_integration;
mod prompt_comprehensive_integrations;
mod prompt_context_integrations;
mod prompt_performance;
mod prompt_real_integrations;
mod sah_serve_integration;
mod sah_serve_tools_validation;
mod todo_clis;
mod var_variables;
mod workflow_parameter_migrations;
mod workflow_shortcuts;
