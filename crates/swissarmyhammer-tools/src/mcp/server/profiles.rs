//! Pre-scoped clones of the running server.
//!
//! A clone shares every piece of the server's state but carries its own tool
//! registry, so the set of tools it serves is fixed at construction rather than
//! filtered per connecting client. Two exist: the validator server, whose
//! registry is the read-only validator profile, and the agent-tools server,
//! whose registry holds the intrinsic agent tools.

use super::McpServer;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::mcp::tool_registry::{register_web_tools, ToolRegistry};
use crate::mcp::tools::agent::register_agent_tools;
use crate::mcp::tools::skill::register_skill_tools;

impl McpServer {
    /// Create a validator-only McpServer clone with a filtered tool registry.
    ///
    /// The returned server shares all state (ToolContext, prompt library, etc.)
    /// but has a separate ToolRegistry containing only validator tools
    /// (`code_context`).
    ///
    /// See [`crate::mcp::tools::register_validator_tools`] for the profile.
    pub fn create_validator_server(&self) -> McpServer {
        // Build the validator registry from the single, data-driven validator
        // profile (`code_context`). The
        // profile is defined once in `tools::register_validator_tools`; this is
        // the only path that interprets it.
        let mut validator_registry = ToolRegistry::new();
        crate::mcp::tools::register_validator_tools(&mut validator_registry);

        let tool_count = validator_registry.len();
        let validator_registry_arc = Arc::new(RwLock::new(validator_registry));

        tracing::debug!(
            "Created validator tool registry with {} validator tools",
            tool_count
        );

        // Clone the tool context but replace its registry with the validator-only one.
        // This prevents validator tools from calling non-validator tools via context.call_tool().
        let mut validator_context = (*self.tool_context).clone();
        validator_context.tool_registry = Some(validator_registry_arc.clone());
        let validator_context = Arc::new(validator_context);

        McpServer {
            library: self.library.clone(),
            file_watcher: self.file_watcher.clone(),
            file_watcher_task: self.file_watcher_task.clone(),
            file_watch_stopped: self.file_watch_stopped.clone(),
            tool_registry: validator_registry_arc,
            tool_context: validator_context,
            skill_library: self.skill_library.clone(),
            agent_library: self.agent_library.clone(),
            work_dir: self.work_dir.clone(),
            tool_config_watcher: self.tool_config_watcher.clone(),
            // The validator registry is already exactly the validator profile
            // (composed by `tools::register_validator_tools`); serve it verbatim
            // rather than re-filtering by host/category, which would apply the
            // primary-only serve gate to its tools.
            compose_per_client: false,
        }
    }

    /// Create an agent-tools-only `McpServer` clone with a filtered registry.
    ///
    /// The returned server shares all state (ToolContext, prompt/skill/agent
    /// libraries, etc.) but has a separate `ToolRegistry` containing only the
    /// tools a base agent needs to be useful: the `Agent`-category tools
    /// (web, skill, subagent). This is the set an ACP agent
    /// with no native tools mounts in-process as its own built-ins, so the agent
    /// is fully tooled even when handed zero external MCP servers.
    ///
    /// # `compose_per_client = false` — load-bearing
    ///
    /// The full server filters its advertised tools per connecting client via
    /// [`Host::serves`](crate::mcp::host::Host::serves), which returns `false` for
    /// every `Agent`-category tool for *every* host (off-the-shelf agents
    /// provide those natively; an agent without them mounts this set itself).
    /// Serving this instance with per-client composition would therefore
    /// advertise **zero** tools to such an agent.
    /// This registry is already exactly the set to serve, so it is served
    /// verbatim — `compose_per_client` is `false`, just like the validator
    /// server.
    pub fn create_agent_tools_server(&self) -> McpServer {
        // `register_file_tools`, `register_web_tools`,
        // `register_agent_tools`, and `register_skill_tools` are imported at
        // module scope.
        let mut agent_registry = ToolRegistry::new();

        register_web_tools(&mut agent_registry);
        register_agent_tools(
            &mut agent_registry,
            self.agent_library.clone(),
            self.library.clone(),
        );
        register_skill_tools(
            &mut agent_registry,
            self.skill_library.clone(),
            self.library.clone(),
        );

        let tool_count = agent_registry.len();
        let agent_registry_arc = Arc::new(RwLock::new(agent_registry));

        tracing::debug!(
            "Created agent-tools registry with {} tools (compose_per_client=false)",
            tool_count
        );

        // Clone the tool context but point it at the agent-only registry so
        // these tools dispatch among themselves, not into the full server.
        let mut agent_context = (*self.tool_context).clone();
        agent_context.tool_registry = Some(agent_registry_arc.clone());
        let agent_context = Arc::new(agent_context);

        McpServer {
            library: self.library.clone(),
            file_watcher: self.file_watcher.clone(),
            file_watcher_task: self.file_watcher_task.clone(),
            file_watch_stopped: self.file_watch_stopped.clone(),
            tool_registry: agent_registry_arc,
            tool_context: agent_context,
            skill_library: self.skill_library.clone(),
            agent_library: self.agent_library.clone(),
            work_dir: self.work_dir.clone(),
            tool_config_watcher: self.tool_config_watcher.clone(),
            // This registry IS the set to serve; serve it verbatim rather than
            // re-filtering by host/category (which strips all `Agent` tools).
            compose_per_client: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_templating::TemplateLibrary;

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_validator_server_has_only_validator_tools() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

        // Full server should have many tools
        let full_tools = server.tool_registry.read().await;
        let full_count = full_tools.len();
        assert!(
            full_count > 4,
            "Full server should have more than 4 tools, got {}",
            full_count
        );
        drop(full_tools);

        // Validator server should expose exactly one tool: code_context.
        let validator = server.create_validator_server();
        let validator_tools = validator.tool_registry.read().await;
        assert_eq!(
            validator_tools.len(),
            1,
            "Validator should have exactly 1 tool (code_context)"
        );

        // Verify the right tool is present.
        assert!(
            validator_tools.get_tool("code_context").is_some(),
            "Validator should have 'code_context' tool"
        );

        // Verify disallowed tools are absent
        assert!(
            validator_tools.get_tool("kanban").is_none(),
            "Validator should NOT have 'kanban' tool"
        );
        assert!(
            validator_tools.get_tool("git").is_none(),
            "Validator should NOT have 'git' tool"
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_validator_context_registry_is_isolated() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();
        let validator = server.create_validator_server();

        // The validator's tool_context should have its own filtered registry
        let validator_ctx_registry = validator
            .tool_context
            .tool_registry
            .as_ref()
            .expect("Validator context should have a tool_registry");
        let registry = validator_ctx_registry.read().await;

        // call_tool on the validator context should NOT find non-validator tools
        assert!(
            registry.get_tool("kanban").is_none(),
            "Validator context registry should not contain 'kanban'"
        );
        assert_eq!(
            registry.len(),
            1,
            "Validator context registry should have exactly 1 tool (code_context)"
        );
    }

    /// Profile audit: the validator server serves *exactly* the validator
    /// profile, no more, no less.
    ///
    /// The validator surface is security-sensitive — the locked-down AVP subset
    /// must not drift. This pins the served tool names to the exact set composed
    /// by `tools::register_validator_tools`. If anyone adds a tool to the
    /// profile (or the validator server registers something extra), this test
    /// fails at the registry boundary.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_validator_server_serves_exactly_the_profile() {
        use std::collections::BTreeSet;

        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();
        let validator = server.create_validator_server();

        let registry = validator.tool_registry.read().await;

        let actual: BTreeSet<&str> = registry
            .iter_tools()
            .map(crate::mcp::tool_registry::McpTool::name)
            .collect();
        let expected: BTreeSet<&str> = ["code_context"].into_iter().collect();

        assert_eq!(
            actual, expected,
            "Validator server must serve exactly the validator profile \
             (code_context)"
        );
    }

    // ---------------------------------------------------------------
    // create_validator_server() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_create_validator_server_shares_work_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let server =
            McpServer::new_with_work_dir(TemplateLibrary::default(), tmp.path().to_path_buf())
                .await
                .unwrap();

        let validator = server.create_validator_server();

        assert_eq!(
            validator.work_dir, server.work_dir,
            "Validator should share the same work_dir"
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_create_validator_server_tool_execution() {
        let tmp = tempfile::tempdir().unwrap();

        let server =
            McpServer::new_with_work_dir(TemplateLibrary::default(), tmp.path().to_path_buf())
                .await
                .unwrap();

        let validator = server.create_validator_server();

        // The validator surface serves `code_context`, so the tool must be
        // addressable and answer a status request.
        let result = validator
            .execute_tool("code_context", serde_json::json!({"op": "get status"}))
            .await;
        assert!(
            result.is_ok(),
            "code_context tool should be available on validator: {:?}",
            result.err()
        );

        // Should NOT be able to execute non-validator tools
        let result = validator
            .execute_tool("kanban", serde_json::json!({}))
            .await;
        assert!(
            result.is_err(),
            "Non-validator tool 'kanban' should not be executable on validator server"
        );
    }
}
