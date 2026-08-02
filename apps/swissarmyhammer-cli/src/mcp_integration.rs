//! Integration layer for calling MCP tools from CLI commands
//!
//! This module provides utilities for CLI commands to call MCP tools directly,
//! eliminating code duplication between CLI and MCP implementations.
//!
//! sah rule ignore test_rule_with_allow

use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::{Map, Value};
use std::sync::Arc;

use swissarmyhammer_agents::AgentLibrary;
use swissarmyhammer_skills::SkillLibrary;
use swissarmyhammer_templating::TemplateLibrary;
use swissarmyhammer_tools::mcp::server::McpServer;
use swissarmyhammer_tools::mcp::tool_config::{apply_tool_config, load_merged_tool_config};
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server_with_options, McpServerMode};
use swissarmyhammer_tools::ToolRegistry;
use swissarmyhammer_tools::{
    register_agent_tools, register_code_context_tools, register_diagnostics_tools,
    register_file_tools, register_git_tools, register_kanban_tools, register_questions_tools,
    register_ralph_tools, register_review_tools, register_shell_tools, register_skill_tools,
    register_web_tools,
};
use tokio::sync::RwLock;

/// CLI-specific tool context that can create and execute MCP tools
pub struct CliToolContext {
    tool_registry: Arc<RwLock<ToolRegistry>>,
    /// MCP server handle (must be kept alive for LlamaAgent to work)
    mcp_server_handle: Option<swissarmyhammer_tools::mcp::unified_server::McpServerHandle>,
    /// In-process server for isolated execution (no HTTP, no env var mutation)
    server: Option<Arc<McpServer>>,
}

impl std::fmt::Debug for CliToolContext {
    /// `ToolRegistry` (holds `Box<dyn McpTool>` trait objects) and `McpServer`
    /// do not implement `Debug`, so they are rendered as opaque placeholders;
    /// only their presence is reported.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliToolContext")
            .field("tool_registry", &"<ToolRegistry>")
            .field("mcp_server_handle", &self.mcp_server_handle)
            .field("server", &self.server.as_ref().map(|_| "<McpServer>"))
            .finish()
    }
}

impl CliToolContext {
    /// Create a new CLI tool context with all necessary storage backends
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let current_dir = std::env::current_dir()?;
        Self::new_with_config(&current_dir, None).await
    }

    /// Create a fully isolated context with no HTTP server and no env var mutation.
    ///
    /// Creates an in-process `McpServer` (the full tool union is registered)
    /// using only the provided working directory. Safe for parallel test execution.
    pub async fn new_isolated(
        working_dir: &std::path::Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mcp_server = McpServer::new_with_work_dir(
            TemplateLibrary::default(),
            working_dir.to_path_buf(),
            None,
        )
        .await?;
        mcp_server.initialize().await?;
        let server_arc = Arc::new(mcp_server);

        let tool_registry = Self::create_tool_registry().await;
        let tool_registry_arc = Arc::new(RwLock::new(tool_registry));

        Ok(Self {
            tool_registry: tool_registry_arc,
            mcp_server_handle: None,
            server: Some(server_arc),
        })
    }

    /// Create a new CLI tool context with optional model override
    ///
    /// # Arguments
    ///
    /// * `working_dir` - The working directory for tool operations
    /// * `model_override` - Optional model name to use for ALL use cases (runtime override)
    ///
    /// # Returns
    ///
    /// Result containing the initialized CliToolContext or an error
    pub async fn new_with_config(
        working_dir: &std::path::Path,
        model_override: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize MCP server with model override
        // The server will create its own tool_context with the correct model configuration
        let mcp_server_handle =
            Self::initialize_mcp_server(model_override, Some(working_dir.to_path_buf())).await?;

        let tool_registry = Self::create_tool_registry().await;
        let tool_registry_arc = Arc::new(RwLock::new(tool_registry));

        Ok(Self {
            tool_registry: tool_registry_arc,
            mcp_server_handle: Some(mcp_server_handle),
            server: None,
        })
    }

    /// Initialize the MCP HTTP server backing this CLI tool context.
    ///
    /// The server registers the full tool union; each tool carries a structural
    /// category consumed by the serve boundary, so no host-conditional
    /// registration happens here.
    async fn initialize_mcp_server(
        model_override: Option<&str>,
        working_dir: Option<std::path::PathBuf>,
    ) -> Result<
        swissarmyhammer_tools::mcp::unified_server::McpServerHandle,
        Box<dyn std::error::Error>,
    > {
        tracing::info!("Starting MCP HTTP server for CLI tool context");

        std::env::set_var("SAH_CLI_MODE", "1");

        let mcp_server_handle = start_mcp_server_with_options(
            McpServerMode::Http { port: None },
            None,
            model_override.map(|s| s.to_string()),
            working_dir,
        )
        .await?;

        tracing::info!(
            "MCP HTTP server ready on port {:?}",
            mcp_server_handle.info().port
        );

        Ok(mcp_server_handle)
    }

    /// Create and populate the tool registry that generates the `sah tool ...`
    /// subcommands.
    ///
    /// Mirrors `swissarmyhammer_tools::mcp::server::McpServer::register_all_tools`
    /// end to end: the same tool families, then the same `tools.yaml` enable and
    /// disable pass. A tool absent here has no CLI command at all, and a tool
    /// the config disables must not get one either — the server refuses to
    /// execute it. `test_cli_tool_registry_matches_server_registry` holds the
    /// two sets equal.
    ///
    /// The skill and agent tools read from libraries. This registry is built
    /// before the `McpServer` exists, so it loads its own libraries from the
    /// builtins rather than sharing the server's.
    ///
    /// `SkillLibrary::load_defaults` and `AgentLibrary::load_defaults` resolve
    /// project skills and agents from the process working directory, not from
    /// any `working_dir` argument.
    async fn create_tool_registry() -> ToolRegistry {
        let mut tool_registry = ToolRegistry::new();
        register_code_context_tools(&mut tool_registry);
        register_diagnostics_tools(&mut tool_registry);
        register_file_tools(&mut tool_registry);
        register_git_tools(&mut tool_registry);
        register_kanban_tools(&mut tool_registry);
        register_questions_tools(&mut tool_registry);
        register_ralph_tools(&mut tool_registry);
        register_shell_tools(&mut tool_registry);
        register_web_tools(&mut tool_registry);
        register_review_tools(&mut tool_registry);

        let prompt_library = Arc::new(RwLock::new(TemplateLibrary::default()));

        let agent_library = Arc::new(RwLock::new(AgentLibrary::new()));
        agent_library.write().await.load_defaults();
        register_agent_tools(&mut tool_registry, agent_library, prompt_library.clone());

        let skill_library = Arc::new(RwLock::new(SkillLibrary::new()));
        skill_library.write().await.load_defaults();
        register_skill_tools(&mut tool_registry, skill_library, prompt_library);

        // Same enable/disable pass the server runs. Without it the CLI offers a
        // command for a tool the server then refuses to execute.
        let tool_config = load_merged_tool_config();
        if !tool_config.disabled_tools().is_empty() {
            apply_tool_config(&mut tool_registry, &tool_config);
        }

        tool_registry
    }

    /// Resolve the McpServer instance from either the isolated server or the HTTP handle
    fn resolve_server(&self) -> Result<Arc<McpServer>, McpError> {
        if let Some(ref server) = self.server {
            return Ok(server.clone());
        }
        self.mcp_server_handle
            .as_ref()
            .and_then(|h| h.server())
            .ok_or_else(|| {
                McpError::internal_error("MCP server instance not available".to_string(), None)
            })
    }

    /// Execute an MCP tool with the given arguments
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Map<String, serde_json::Value>,
    ) -> Result<CallToolResult, McpError> {
        let server = self.resolve_server()?;
        server
            .execute_tool(tool_name, serde_json::Value::Object(arguments))
            .await
    }

    /// Get an Arc to the tool registry for dynamic CLI generation
    pub fn get_tool_registry_arc(&self) -> Arc<RwLock<ToolRegistry>> {
        self.tool_registry.clone()
    }

    /// Create arguments map from any iterable of key-value pairs.
    ///
    /// Accepts `impl IntoIterator` so callers may pass a `Vec`, an array
    /// literal, or any other iterator without first allocating a `Vec`.
    pub fn create_arguments<'a>(
        &self,
        args: impl IntoIterator<Item = (&'a str, Value)>,
    ) -> Map<String, Value> {
        args.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }
}

/// Utilities for formatting MCP responses for CLI display
pub mod response_formatting {
    use rmcp::model::{CallToolResult, RawContent};
    use serde_json::Value;

    /// Format successful tool result for display.
    ///
    /// This is the ONE PLACE where we convert JSON output to YAML for display.
    pub fn format_success_response(result: &CallToolResult) -> String {
        // First check if there's structured content - serialize it to YAML
        if let Some(ref data) = result.structured_content {
            return serde_yaml_ng::to_string(data).unwrap_or_else(|_| {
                serde_json::to_string_pretty(data)
                    .unwrap_or_else(|_| "Operation successful".to_string())
            });
        }

        // Try to extract text content and parse as JSON, then convert to YAML
        if let Ok(json_value) = extract_json_data(result) {
            // Successfully parsed as JSON - convert to YAML with leading newline
            let text = extract_text_content(result).unwrap_or_default();
            return serde_yaml_ng::to_string(&json_value)
                .map(|yaml| format!("\n{}", yaml))
                .unwrap_or(text);
        }

        // Not JSON or no content - return raw text or default
        extract_text_content(result).unwrap_or_else(|| "Operation successful".to_string())
    }

    /// Format a successful tool result as exactly one JSON document.
    ///
    /// Used for tools that report `McpTool::cli_output_is_json`, whose output
    /// is strict-parsed by a program. Every branch returns valid JSON — even
    /// the one where the tool answered with plain text, which becomes a JSON
    /// string — so a consumer can always load stdout. Nothing is prepended:
    /// the leading newline the YAML rendering carries would break the parse.
    pub fn format_success_response_json(result: &CallToolResult) -> String {
        if let Some(ref data) = result.structured_content {
            return to_pretty_json(data);
        }

        if let Ok(json_value) = extract_json_data(result) {
            return to_pretty_json(&json_value);
        }

        let text =
            extract_text_content(result).unwrap_or_else(|| "Operation successful".to_string());
        to_pretty_json(&Value::String(text))
    }

    /// Serialize a [`Value`] as pretty JSON.
    ///
    /// A `Value` holds no type that can fail to serialize, so the fallback is
    /// unreachable; `null` keeps the output parseable if it ever is reached.
    fn to_pretty_json(value: &Value) -> String {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_string())
    }

    /// Format error tool result for display
    pub fn format_error_response(result: &CallToolResult) -> String {
        extract_text_content(result).unwrap_or_else(|| "Operation failed".to_string())
    }

    /// Extract text content from CallToolResult
    pub fn extract_text_content(result: &CallToolResult) -> Option<String> {
        result
            .content
            .first()
            .and_then(|content| match &content.raw {
                RawContent::Text(text_content) => Some(text_content.text.clone()),
                _ => None,
            })
    }

    /// Extract JSON data from CallToolResult
    pub fn extract_json_data(result: &CallToolResult) -> Result<Value, Box<dyn std::error::Error>> {
        let text = extract_text_content(result).ok_or("No text content found in result")?;
        Ok(serde_json::from_str(&text)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

    /// `CliToolContext::new()` reads process-global CWD via
    /// `std::env::current_dir()` and roots an `McpServer` at whatever it
    /// observes. `#[serial_test::serial(cwd)]` joins the crate-wide `cwd`
    /// group so this CWD-reading test cannot run while another test
    /// (`skill.rs`, `registry.rs`, `doctor/checks.rs`)
    /// is mid-`set_current_dir`/`CurrentDirGuard` and the observed CWD would
    /// be a tempdir about to be dropped.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_cli_tool_context_creation() {
        // Isolate HOME + CWD — `CliToolContext::new()` resolves the project
        // directory from cwd and creates `.sah/` there as a side effect.
        // Without isolation this leaks a `.sah/` skeleton into the host crate
        // directory.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let result = CliToolContext::new().await;
        assert!(
            result.is_ok(),
            "Failed to create CliToolContext: {:?}",
            result.err()
        );

        let _context = result.unwrap();
        // Context creation successful - this verifies the tool registry is working
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_create_arguments() {
        // Isolate CWD — `CliToolContext::new_isolated` still reaches into
        // cwd-based path resolution for some sub-context and creates `.sah/`
        // at cwd. Without the CWD guard the empty `.sah/` directory leaks
        // into the host crate directory.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let temp = tempfile::TempDir::new().unwrap();
        let context = CliToolContext::new_isolated(temp.path()).await.unwrap();

        let args = context.create_arguments(vec![("name", json!("test")), ("count", json!(42))]);

        assert_eq!(args.get("name"), Some(&json!("test")));
        assert_eq!(args.get("count"), Some(&json!(42)));
    }

    /// `create_arguments` accepts any `IntoIterator`, not just `Vec`, so callers
    /// can pass an array literal without allocating a `Vec` first.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_create_arguments_accepts_array() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let temp = tempfile::TempDir::new().unwrap();
        let context = CliToolContext::new_isolated(temp.path()).await.unwrap();

        // Array literal (`[..]`), not `vec![..]` — only compiles if the
        // signature takes `impl IntoIterator`.
        let args = context.create_arguments([("name", json!("test")), ("count", json!(42))]);

        assert_eq!(args.get("name"), Some(&json!("test")));
        assert_eq!(args.get("count"), Some(&json!(42)));
    }

    /// `CliToolContext` must implement `Debug` so it can appear in diagnostic
    /// output and `#[derive(Debug)]` on enclosing types. Its field types
    /// (`ToolRegistry`, `McpServer`) do not implement `Debug`, so this is a
    /// manual impl with placeholders — this test just exercises that it
    /// formats without panicking.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_cli_tool_context_implements_debug() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let temp = tempfile::TempDir::new().unwrap();
        let context = CliToolContext::new_isolated(temp.path()).await.unwrap();

        let debug = format!("{context:?}");
        assert!(
            debug.contains("CliToolContext"),
            "Debug output should name the struct, got: {debug}"
        );
    }

    #[test]
    fn test_response_formatting() {
        use rmcp::model::Content;

        let success_result =
            CallToolResult::success(vec![Content::text("Operation successful".to_string())]);

        let formatted = response_formatting::format_success_response(&success_result);
        assert!(formatted.contains("Operation successful"));

        // Verify extract_json_data works on non-JSON text
        let result = response_formatting::extract_json_data(&success_result);
        assert!(result.is_err(), "Non-JSON text should fail to parse");
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_isolated_tool_execution() {
        // Isolate CWD — see `test_create_arguments`.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let temp = tempfile::TempDir::new().unwrap();
        let context = CliToolContext::new_isolated(temp.path()).await.unwrap();

        let args = context.create_arguments(vec![
            ("op", json!("add task")),
            ("title", json!("Test task")),
            ("description", json!("Test context")),
        ]);

        let result = context.execute_tool("kanban", args).await;

        match result {
            Ok(_) => {}
            Err(e) => {
                let error_str = e.to_string();
                assert!(
                    !error_str.contains("rate limit"),
                    "Should not fail due to rate limiting in normal usage: {error_str}"
                );
            }
        }
    }

    /// Tools the CLI registry deliberately omits, each paired with the reason.
    ///
    /// The list is empty on purpose: every MCP tool is reachable as
    /// `sah tool <tool_name> ...`. An entry here must carry a reason, so that a
    /// tool can never drop out of the CLI silently again.
    const CLI_REGISTRY_EXCLUSIONS: &[(&str, &str)] = &[];

    /// The CLI builds its own tool registry to generate the `sah tool ...`
    /// subcommands. It must hold the same tools the MCP server registers, and
    /// every one of them must reach the command line.
    ///
    /// The two lists drifted once: the server registered `ralph`, `agent`,
    /// `diagnostics` and `skill` and the CLI did not, so
    /// `sah tool ralph ralph check --` failed with `unrecognized subcommand`
    /// and the ralph Stop hook broke. A doc comment that said the two "should
    /// mirror" did not hold; this test does.
    ///
    /// Joins the crate-wide `cwd` group: it builds an `McpServer`, which reads
    /// process-global CWD (see `test_cli_tool_context_creation`).
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_cli_tool_registry_matches_server_registry() {
        use std::collections::BTreeSet;
        use swissarmyhammer_tools::mcp::tool_registry::McpTool;

        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let cli_registry = CliToolContext::create_tool_registry().await;
        let cli_names: BTreeSet<String> = cli_registry.list_tool_names().into_iter().collect();
        // A registered tool without a CLI category never becomes a
        // subcommand, so registration alone does not put it on the CLI.
        let commanded: BTreeSet<String> = cli_registry
            .get_cli_tools()
            .iter()
            .map(|tool| McpTool::name(*tool).to_string())
            .collect();

        // The server registry is the reference set. Build it the way the
        // server does, through `register_all_tools`.
        let server = McpServer::new_with_work_dir(TemplateLibrary::default(), env.temp_dir(), None)
            .await
            .expect("mcp server");
        let server_names: BTreeSet<String> = server
            .get_tool_registry()
            .read()
            .await
            .list_tool_names()
            .into_iter()
            .collect();

        let excluded: BTreeSet<&str> = CLI_REGISTRY_EXCLUSIONS
            .iter()
            .map(|(name, _reason)| *name)
            .collect();

        let missing: Vec<&str> = server_names
            .difference(&cli_names)
            .map(String::as_str)
            .filter(|name| !excluded.contains(name))
            .collect();
        let extra: Vec<&str> = cli_names
            .difference(&server_names)
            .map(String::as_str)
            .collect();
        let uncommanded: Vec<&str> = cli_names
            .difference(&commanded)
            .map(String::as_str)
            .filter(|name| !excluded.contains(name))
            .collect();

        assert!(
            missing.is_empty() && extra.is_empty() && uncommanded.is_empty(),
            "CLI tool registry drifted from the MCP server registry.\n\
             Registered by the server, missing from the CLI: {missing:?}\n\
             Registered by the CLI only: {extra:?}\n\
             Registered by the CLI but with no `sah tool` command \
             (needs `cli_category`): {uncommanded:?}\n\
             Fix `CliToolContext::create_tool_registry`, or name the tool in \
             CLI_REGISTRY_EXCLUSIONS with a reason."
        );
    }

    /// Validates that all registered tools pass CLI validation.
    ///
    /// This test uses the same code path as the actual CLI (CliToolContext::new())
    /// to ensure the test validates the real tool registration, not a separate copy.
    /// If this test fails, it means a tool was added without proper schema validation.
    ///
    /// Joins the crate-wide `cwd` group: `CliToolContext::new()` reads
    /// process-global CWD (see `test_cli_tool_context_creation`).
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_all_registered_tools_pass_cli_validation() {
        use crate::dynamic_cli::CliBuilder;

        // Isolate HOME + CWD — see `test_cli_tool_context_creation`.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        // Use the same code path as the actual CLI
        let context = CliToolContext::new()
            .await
            .expect("Failed to create CliToolContext");
        let tool_registry_arc = context.get_tool_registry_arc();

        // Create CLI builder and validate all tools
        let cli_builder = CliBuilder::new(tool_registry_arc);
        let validation_errors = cli_builder.validate_all_tools();

        // If there are validation errors, fail with detailed messages
        if !validation_errors.is_empty() {
            let error_messages: Vec<String> =
                validation_errors.iter().map(|e| e.to_string()).collect();
            panic!(
                "Tool validation failed! All registered tools must have valid schemas for CLI generation.\n\
                 Validation errors:\n  - {}",
                error_messages.join("\n  - ")
            );
        }

        // Also verify the stats show all tools are valid
        let stats = cli_builder.get_validation_stats();
        assert!(
            stats.is_all_valid(),
            "Expected all tools to be valid. Stats: {}",
            stats.summary()
        );
        assert!(
            stats.total_tools > 0,
            "Expected at least one tool to be registered"
        );
    }
}
