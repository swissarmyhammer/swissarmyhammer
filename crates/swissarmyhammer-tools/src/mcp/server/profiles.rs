//! Pre-scoped clones of the running server, and the native tools they supersede.
//!
//! A clone shares every piece of the server's state but carries its own tool
//! registry, so the set of tools it serves is fixed at construction rather than
//! filtered per connecting client. Two exist: the validator server, whose
//! registry is the read-only validator profile, and the agent-tools server,
//! whose registry holds the intrinsic agent tools.

use super::McpServer;
use crate::mcp::host::Host;
use rmcp::model::*;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::mcp::tool_registry::{
    register_file_tools, register_shell_tools, register_web_tools, ToolRegistry,
};
use crate::mcp::tools::agent::register_agent_tools;
use crate::mcp::tools::skill::register_skill_tools;

impl McpServer {
    /// Create a validator-only McpServer clone with a filtered tool registry.
    ///
    /// The returned server shares all state (ToolContext, prompt library, etc.)
    /// but has a separate ToolRegistry containing only validator tools
    /// (code_context + the unified read-only `files` tool).
    ///
    /// The validator file surface is the read-only variant of the unified
    /// `files` tool ([`FilesTool::read_only`](crate::mcp::tools::files::FilesTool::read_only)):
    /// `read file` / `glob files` / `grep files` ops only, no `write`/`edit`.
    /// See [`crate::mcp::tools::register_validator_tools`] for the profile.
    pub fn create_validator_server(&self) -> McpServer {
        // Build the validator registry from the single, data-driven validator
        // profile (code_context + the unified read-only `files` tool). The
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
            // primary-only serve gate to its read-only `files` tool.
            compose_per_client: false,
            // Non-primary instance: the serve-time deny is gated on
            // `compose_per_client`, so this latch is never read. Its own flag.
            bash_denied: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Create an agent-tools-only `McpServer` clone with a filtered registry.
    ///
    /// The returned server shares all state (ToolContext, prompt/skill/agent
    /// libraries, etc.) but has a separate `ToolRegistry` containing only the
    /// tools a base agent needs to be useful: the `Agent`-category tools
    /// (the unified `files` tool — read/write/edit/glob/grep — web, skill,
    /// subagent) plus the shell `Replacement` tool. This is the set an ACP agent
    /// with no native tools mounts in-process as its own built-ins, so the agent
    /// is fully tooled even when handed zero external MCP servers.
    ///
    /// Files are served through the single unified `files` tool (CLI-style `op`
    /// dispatch), which keeps `write`/`edit` for the agent. The by-name split
    /// forms are not registered.
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
        // `register_file_tools`, `register_web_tools`, `register_shell_tools`,
        // `register_agent_tools`, and `register_skill_tools` are imported at
        // module scope.
        let mut agent_registry = ToolRegistry::new();

        // Files: the unified `op`-dispatched tool, which keeps write/edit/read/
        // glob/grep for the agent.
        register_file_tools(&mut agent_registry);
        register_web_tools(&mut agent_registry);
        // Shell is the `Replacement{native:"Bash"}` tool; an agent mounting this
        // set gets its shell from here (and only here), satisfying the "shell
        // appears exactly once" invariant.
        register_shell_tools(&mut agent_registry);
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
            // Non-primary instance (the in-process agent-tools mount): never
            // fires the serve-time deny (gated on `compose_per_client`).
            bash_denied: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Suppress the native host tools the served `Replacement` tools supersede,
    /// for a Claude client connecting at serve time.
    ///
    /// A `Replacement { native }` tool (today only `shell`, replacing `"Bash"`)
    /// is served to Claude so it supersedes Claude's native tool of that name.
    /// For the supersession to be real, Claude's native must also be *denied* —
    /// otherwise the model sees both and may keep reaching for the native. This
    /// writes that deny into Claude's local settings via the same idempotent
    /// mirdan primitive `sah init` already uses ([`mirdan::install::deny_tool`]),
    /// deriving the native names from the registry's `Replacement` categories
    /// rather than hardcoding `"Bash"`, so the suppression tracks the served set.
    ///
    /// Gates:
    /// - **Primary serve only.** Skips when `compose_per_client` is `false` (the
    ///   validator and agent-tools-mount instances), which never advertise the
    ///   `Replacement` tools and so must not write denies.
    /// - **Claude only.** Unknown clients keep their native tools;
    ///   re-allowing is left to `deinit`, not self-corrected here.
    /// - **Once.** Latches on first Claude connection so repeated `initialize`s
    ///   don't rewrite settings; the underlying deny is idempotent regardless.
    ///
    /// Scope is [`InitScope::Local`] — Claude's `.claude/settings.local.json`,
    /// resolved from the serve working directory — so the deny is a runtime/local
    /// change, not a committed repo edit. Reports through a `tracing`-backed
    /// reporter because serve-path stderr is swallowed by the MCP transport.
    pub(super) async fn apply_serve_time_native_deny(&self, client_info: &Implementation) {
        use std::sync::atomic::Ordering;
        use swissarmyhammer_common::lifecycle::InitScope;
        use swissarmyhammer_common::reporter::TracingReporter;

        // Only the primary per-client serve instance suppresses natives; the
        // validator and agent-tools-mount instances serve pre-scoped registries and
        // must never write agent settings.
        if !self.compose_per_client {
            return;
        }

        if Host::from_client_info(client_info) != Host::Claude {
            return;
        }

        // Latch: first Claude connection wins; later ones are no-ops.
        if self.bash_denied.swap(true, Ordering::SeqCst) {
            return;
        }

        let natives = {
            let registry = self.tool_registry.read().await;
            registry.replacement_natives()
        };

        let reporter = TracingReporter;
        for native in natives {
            let results = mirdan::install::deny_tool(InitScope::Local, native, &reporter);
            for r in &results {
                if r.status == swissarmyhammer_common::lifecycle::InitStatus::Error {
                    tracing::warn!(
                        native = native,
                        "serve-time native deny error: {} — {}",
                        r.name,
                        r.message
                    );
                }
            }
            tracing::info!(
                native = native,
                client = %client_info.name,
                event = "serve_native_denied",
                "Denied native host tool for Claude (replaced by served tool)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::server::handler::format_call_result_text;
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

        // Validator server should expose exactly two tools:
        // code_context plus the unified read-only `files` tool.
        let validator = server.create_validator_server();
        let validator_tools = validator.tool_registry.read().await;
        assert_eq!(
            validator_tools.len(),
            2,
            "Validator should have exactly 2 tools (code_context + files)"
        );

        // Verify the right tools are present.
        assert!(
            validator_tools.get_tool("files").is_some(),
            "Validator should have the unified 'files' tool"
        );
        assert!(
            validator_tools.get_tool("code_context").is_some(),
            "Validator should have 'code_context' tool"
        );

        // The former split by-name tools must NOT be served on the validator
        // endpoint — only the unified op-dispatched `files` tool.
        assert!(
            validator_tools.get_tool("read_file").is_none(),
            "Validator must NOT have the split 'read_file' tool"
        );
        assert!(
            validator_tools.get_tool("glob_files").is_none(),
            "Validator must NOT have the split 'glob_files' tool"
        );
        assert!(
            validator_tools.get_tool("grep_files").is_none(),
            "Validator must NOT have the split 'grep_files' tool"
        );

        // Verify disallowed tools are absent
        assert!(
            validator_tools.get_tool("kanban").is_none(),
            "Validator should NOT have 'kanban' tool"
        );
        assert!(
            validator_tools.get_tool("shell").is_none(),
            "Validator should NOT have 'shell' tool"
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
        // The unified `files` tool is present; the former split tools are not.
        assert!(
            registry.get_tool("files").is_some(),
            "Validator context registry should contain the unified 'files' tool"
        );
        assert!(
            registry.get_tool("read_file").is_none(),
            "Validator context registry must NOT contain the split 'read_file' tool"
        );
        assert!(
            registry.get_tool("glob_files").is_none(),
            "Validator context registry must NOT contain the split 'glob_files' tool"
        );
        assert!(
            registry.get_tool("grep_files").is_none(),
            "Validator context registry must NOT contain the split 'grep_files' tool"
        );
        assert_eq!(
            registry.len(),
            2,
            "Validator context registry should have exactly 2 tools (code_context + files)"
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
        let expected: BTreeSet<&str> = ["code_context", "files"].into_iter().collect();

        assert_eq!(
            actual, expected,
            "Validator server must serve exactly the validator profile \
             (code_context + files)"
        );
    }

    /// Read-only enforcement + op-dispatched behavior audit (#4 in task
    /// description).
    ///
    /// The validator endpoint exposes the unified, read-only `files` tool —
    /// `read file` / `glob files` / `grep files` ops only. The `write file` and
    /// `edit file` ops are rejected, and the former split by-name tools
    /// (`read_file`, `glob_files`, `grep_files`) are not addressable at all.
    /// This test asserts every half of that contract by EXECUTING real op
    /// calls through the validator-facing server and inspecting the output:
    ///
    /// 1. Op-dispatched `read file` / `glob files` / `grep files` succeed and
    ///    return the expected real content.
    /// 2. Op-dispatched `write file` / `edit file` are rejected (read-only),
    ///    and the file on disk is unchanged.
    /// 3. The former split by-name tool names are Unknown to the registry.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_validator_files_tool_is_read_only_and_op_dispatched() {
        let tmp = tempfile::tempdir().unwrap();
        let test_file = tmp.path().join("test.txt");
        std::fs::write(&test_file, "hello world").unwrap();

        let server =
            McpServer::new_with_work_dir(TemplateLibrary::default(), tmp.path().to_path_buf())
                .await
                .unwrap();

        let validator = server.create_validator_server();

        // 1a. Op-dispatched `read file` succeeds and returns the file content.
        let read_result = validator
            .execute_tool(
                "files",
                serde_json::json!({
                    "op": "read file",
                    "path": test_file.to_str().unwrap(),
                }),
            )
            .await
            .expect("op-dispatched 'read file' must succeed on the validator server");
        let (_, read_text) = format_call_result_text(&read_result);
        assert!(
            read_text.contains("hello world"),
            "read output must contain the real file content; got: {read_text}"
        );

        // 1b. Op-dispatched `glob files` succeeds and finds the test file.
        let glob_result = validator
            .execute_tool(
                "files",
                serde_json::json!({
                    "op": "glob files",
                    "pattern": "*.txt",
                    "path": tmp.path().to_str().unwrap(),
                }),
            )
            .await
            .expect("op-dispatched 'glob files' must succeed on the validator server");
        let (_, glob_text) = format_call_result_text(&glob_result);
        assert!(
            glob_text.contains("test.txt"),
            "glob output must list the matched file; got: {glob_text}"
        );

        // 1c. Op-dispatched `grep files` succeeds and finds the matching line.
        let grep_result = validator
            .execute_tool(
                "files",
                serde_json::json!({
                    "op": "grep files",
                    "pattern": "hello",
                    "path": tmp.path().to_str().unwrap(),
                }),
            )
            .await
            .expect("op-dispatched 'grep files' must succeed on the validator server");
        let (_, grep_text) = format_call_result_text(&grep_result);
        assert!(
            grep_text.contains("hello"),
            "grep output must contain the matched text; got: {grep_text}"
        );

        // 2. Write/edit ops are rejected by the read-only surface.
        let write_result = validator
            .execute_tool(
                "files",
                serde_json::json!({
                    "op": "write file",
                    "file_path": tmp.path().join("written.txt").to_str().unwrap(),
                    "content": "should not be written",
                }),
            )
            .await;
        assert!(
            write_result.is_err(),
            "'write file' op must be rejected on the read-only validator surface"
        );

        let edit_result = validator
            .execute_tool(
                "files",
                serde_json::json!({
                    "op": "edit file",
                    "file_path": test_file.to_str().unwrap(),
                    "old_string": "hello",
                    "new_string": "goodbye",
                }),
            )
            .await;
        assert!(
            edit_result.is_err(),
            "'edit file' op must be rejected on the read-only validator surface"
        );

        // The file must remain unchanged — write/edit ops are refused before
        // touching disk.
        assert_eq!(
            std::fs::read_to_string(&test_file).unwrap(),
            "hello world",
            "rejected write/edit ops must not modify the file on disk"
        );

        // 3. The former split by-name tools are not addressable.
        for split in ["read_file", "glob_files", "grep_files"] {
            let err = validator
                .execute_tool(split, serde_json::json!({}))
                .await
                .expect_err("split by-name tools must not be registered on the validator server");
            let msg = format!("{:?}", err);
            assert!(
                msg.contains("unknown tool"),
                "validator should reject '{split}' as an unknown tool; got: {msg}"
            );
        }
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
        let test_file = tmp.path().join("hello.txt");
        std::fs::write(&test_file, "hi").unwrap();

        let server =
            McpServer::new_with_work_dir(TemplateLibrary::default(), tmp.path().to_path_buf())
                .await
                .unwrap();

        let validator = server.create_validator_server();

        // Should be able to execute a validator file read via the unified
        // op-dispatched `files` tool. The split by-name tools are no longer
        // registered; the validator surface is the unified read-only `files`.
        let result = validator
            .execute_tool(
                "files",
                serde_json::json!({"op": "read file", "path": test_file.to_str().unwrap()}),
            )
            .await;
        if let Err(e) = &result {
            let msg = format!("{:?}", e);
            assert!(
                !msg.contains("unknown tool"),
                "files tool should be available on validator: {}",
                msg
            );
        }

        // Should NOT be able to execute non-validator tools
        let result = validator.execute_tool("shell", serde_json::json!({})).await;
        assert!(
            result.is_err(),
            "Non-validator tool 'shell' should not be executable on validator server"
        );
    }
}
