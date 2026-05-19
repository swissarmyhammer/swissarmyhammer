//! Integration tests for exposing the in-process MCP tools to the plugin
//! platform.
//!
//! The MCP server bootstrap builds a [`ToolRegistry`] of in-process tools and
//! then hands each one to a [`PluginHost`] as a Rust module via
//! `expose_rust_module`. These tests drive that wiring end to end: a real
//! plugin bundle activates an exposed module under a name of its choosing with
//! `register(name, { rust: id })`, and a call routed through the host observes
//! a genuine effect — a `files` write that lands a real file on disk.
//!
//! Every cross-thread interaction is bounded by a timeout so a wedged isolate
//! fails the test fast instead of hanging CI.

use std::time::Duration;

use serde_json::json;
use swissarmyhammer_plugin::{CallerId, PluginHost};
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::McpServer;

/// A generous upper bound on any single host interaction.
const TIMEOUT: Duration = Duration::from_secs(30);

/// The well-known `_meta` key carrying the operation discovery tree.
const OPERATIONS_META_KEY: &str = "io.swissarmyhammer/operations";

/// Writes a one-file plugin bundle whose `load` export runs `body`.
///
/// The entry is the bundle's `index.ts`: it imports the SDK, declares a
/// `Plugin` subclass whose `load` contains `body`, and exports a `load`
/// lifecycle function — the bundle shape the host's `load(plugin_dir)` expects.
fn write_plugin(dir: &std::path::Path, body: &str) {
    let entry = format!(
        "import {{ Plugin, makePluginThis }} from '@swissarmyhammer/plugin';\n\
         class P extends Plugin {{\n\
           async load(): Promise<void> {{\n{body}\n}}\n\
         }}\n\
         export async function load(): Promise<unknown> {{\n\
           const p = makePluginThis(new P()) as P;\n\
           await p.load();\n\
           return null;\n\
         }}\n"
    );
    std::fs::write(dir.join("index.ts"), entry).expect("index.ts should be written");
}

/// Builds an MCP server against an isolated temp working directory.
///
/// The temp dir keeps `initialize_code_context` from walking the real
/// monorepo, and gives the `files` tool a clean place to write.
///
/// `agent_mode` is `true` so the full in-process tool set — including the
/// unified `files` tool, which is an agent tool — is registered.
async fn build_mcp_server(work_dir: &std::path::Path) -> McpServer {
    McpServer::new_with_work_dir(PromptLibrary::new(), work_dir.to_path_buf(), None, true)
        .await
        .expect("MCP server bootstrap should succeed")
}

/// Building the MCP server and exposing its tools puts the in-process tools —
/// `files` and `kanban` among them — into the plugin platform's
/// available-modules table.
#[tokio::test]
async fn bootstrap_exposes_in_process_tools_as_rust_modules() {
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let plugin_root = tempfile::TempDir::new().expect("plugin root temp");

    let server = build_mcp_server(work_dir.path()).await;
    let host = PluginHost::for_tests(plugin_root.path().to_path_buf(), None);

    tokio::time::timeout(TIMEOUT, server.expose_tools_to_plugin_host(&host))
        .await
        .expect("exposing tools should not hang")
        .expect("exposing the in-process tools should succeed");

    // The available-modules table is observable through activation: a plugin
    // that registers `{ rust: "files" }` and `{ rust: "kanban" }` succeeds
    // only when both modules were exposed.
    let bundle = tempfile::TempDir::new().expect("bundle temp");
    write_plugin(
        bundle.path(),
        "this.register('files-mod', { rust: 'files' });\n\
         this.register('kanban-mod', { rust: 'kanban' });",
    );

    tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("activating the exposed files and kanban modules should succeed");
}

/// After a plugin activates the exposed `files` module with
/// `register(name, { rust: "files" })`, a `write file` call routed through the
/// host writes a real file to disk.
#[tokio::test]
async fn activated_files_module_writes_a_real_file_through_the_dispatcher() {
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let plugin_root = tempfile::TempDir::new().expect("plugin root temp");

    let server = build_mcp_server(work_dir.path()).await;
    let host = PluginHost::for_tests(plugin_root.path().to_path_buf(), None);

    tokio::time::timeout(TIMEOUT, server.expose_tools_to_plugin_host(&host))
        .await
        .expect("exposing tools should not hang")
        .expect("exposing the in-process tools should succeed");

    // A plugin activates the exposed `files` module under the name `fs`.
    let bundle = tempfile::TempDir::new().expect("bundle temp");
    write_plugin(bundle.path(), "this.register('fs', { rust: 'files' });");
    tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("activating the exposed files module should succeed");

    // Route a `write file` call through the host's dispatcher and assert the
    // real effect: the file exists on disk with the written content.
    let target = work_dir.path().join("plugin_written.txt");
    let result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "fs",
            "files",
            json!({
                "op": "write file",
                "file_path": target.to_string_lossy(),
                "content": "written via the plugin platform",
            }),
        ),
    )
    .await
    .expect("the dispatched call should not hang")
    .expect("the dispatched files write should succeed");

    assert!(
        !result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "the files write should not report an error, got {result}"
    );
    let written = std::fs::read_to_string(&target).expect("the file should exist on disk");
    assert_eq!(
        written, "written via the plugin platform",
        "the dispatched files write should land the real content on disk"
    );
}

/// An exposed operation tool advertises the `io.swissarmyhammer/operations`
/// discovery `_meta` on the tool definition its `tools()` publishes.
#[tokio::test]
async fn exposed_operation_tool_carries_operations_meta() {
    use swissarmyhammer_plugin::McpServer as PluginMcpServer;

    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let server = build_mcp_server(work_dir.path()).await;

    // The exposure glue hands one platform `McpServer` per in-process tool.
    let modules = server.plugin_tool_modules().await;
    let (_, files_module) = modules
        .iter()
        .find(|(id, _)| id == "files")
        .expect("the `files` tool should be exposed as a Rust module");

    let tools = PluginMcpServer::tools(files_module.as_ref());
    let files_tool = tools
        .iter()
        .find(|t| t.name() == "files")
        .expect("the files module should publish a `files` tool");

    let meta = files_tool
        .as_tool()
        .meta
        .as_ref()
        .expect("an operation tool should publish `_meta`");
    let ops_meta = meta
        .0
        .get(OPERATIONS_META_KEY)
        .unwrap_or_else(|| panic!("`_meta` should carry the `{OPERATIONS_META_KEY}` key"));

    // The discovery tree is keyed by noun; `files` operations are nouned
    // `file` / `files`, so the tree is a non-empty object.
    assert!(
        ops_meta.as_object().is_some_and(|tree| !tree.is_empty()),
        "the operations `_meta` should be a non-empty noun-keyed tree, got {ops_meta}"
    );
}
