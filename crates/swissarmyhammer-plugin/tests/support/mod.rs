//! Shared test-support module for the example-plugin end-to-end tests.
//!
//! Every `*_e2e.rs` test that exercises a committed example plugin pulls this
//! module in with `mod support;`. Cargo compiles only top-level `tests/*.rs`
//! files into their own test binaries — a nested `tests/support/mod.rs` is
//! never a binary of its own, so this file is shared source rather than a
//! standalone target.
//!
//! The helpers here are the seam between the committed example bundles under
//! `examples/plugins/` and the real plugin platform: [`examples_root`] locates
//! the bundles, [`stage_example`] copies one into a temp layer root so it can
//! be discovered, [`build_mcp_server`] stands up the real in-process MCP tool
//! registry, and [`expose_kanban_module`] exposes the in-process `kanban`
//! operation tool over a temp board so an example can drive it. Each test file
//! uses only a subset of them, so the helpers carry `#[allow(dead_code)]`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_plugin::{CallerId, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR};
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::plugin_bridge::build_tool_modules;
use swissarmyhammer_tools::mcp::{McpServer, ToolHandlers};
use swissarmyhammer_tools::{register_kanban_tools, ToolContext, ToolRegistry};
use tokio::sync::{Mutex as TokioMutex, RwLock};

/// A generous upper bound on any single host or server interaction.
///
/// Building the MCP server stands up the full in-process tool registry, so the
/// bound is wider than a bare isolate test would need. It mirrors the `TIMEOUT`
/// const the reference `files_dispatch_e2e` test uses.
#[allow(dead_code)]
pub const TIMEOUT: Duration = Duration::from_secs(60);

/// The directory holding the committed example plugin bundles.
///
/// Resolves to `<CARGO_MANIFEST_DIR>/examples/plugins`, the home for every
/// example bundle this crate ships. A bundle lives in a `<name>/` subdirectory
/// of this root and carries a real `plugin.json` plus its entry module.
///
/// # Returns
///
/// The absolute path to `examples/plugins/` inside this crate.
#[allow(dead_code)]
pub fn examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/plugins")
}

/// Stages a committed example bundle into a temp layer root for discovery.
///
/// Recursively copies the committed bundle `examples_root()/<name>/` into
/// `<layer_root>/plugins/<name>/`, so a test can point a [`PluginHost`] layer
/// at `layer_root` and have discovery find a real bundle. The committed
/// `examples/plugins/` tree stays read-only — only the temp copy is touched.
///
/// [`PluginHost`]: swissarmyhammer_plugin::PluginHost
///
/// # Parameters
///
/// - `name` — the example bundle directory name under `examples/plugins/`.
/// - `layer_root` — the temp layer root to stage the bundle beneath; the
///   bundle lands at `<layer_root>/plugins/<name>/`.
///
/// # Panics
///
/// Panics if the source bundle does not exist or any filesystem copy fails —
/// a staging failure is a test setup error, not a condition under test.
#[allow(dead_code)]
pub fn stage_example(name: &str, layer_root: &Path) {
    let source = examples_root().join(name);
    assert!(
        source.is_dir(),
        "example bundle '{name}' must exist at {} to be staged",
        source.display(),
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join(name);
    copy_dir_recursive(&source, &destination);
}

/// Stages a committed example bundle, then rewrites placeholder tokens in it.
///
/// Behaves exactly like [`stage_example`] — recursively copying the committed
/// bundle into `<layer_root>/plugins/<name>/` — and then performs literal
/// string replacement across **every staged file** for each
/// `(token, replacement)` pair in `substitutions`.
///
/// This is the seam an example needs when its `entry.ts` must reference a value
/// that cannot be known at authoring time — most notably an absolute binary
/// path. The COMMITTED bundle stays a clean, readable example carrying a named
/// placeholder token; only the throwaway staged temp copy is specialized with
/// the real value, so the committed source is never an unrunnable fragment.
///
/// Substitutions are applied to the staged copy only — the committed
/// `examples/plugins/` tree is never touched.
///
/// This helper supports **text bundles only**: every staged file is read as
/// UTF-8 to scan for tokens, so a bundle carrying a non-UTF-8 binary asset
/// would panic. Every example bundle today is `plugin.json` + `entry.ts`, both
/// text — a binary-asset bundle would need a different staging path.
///
/// # Parameters
///
/// - `name` — the example bundle directory name under `examples/plugins/`.
/// - `layer_root` — the temp layer root to stage the bundle beneath; the
///   bundle lands at `<layer_root>/plugins/<name>/`.
/// - `substitutions` — `(token, replacement)` pairs; each `token` found in any
///   staged file is replaced with its `replacement`.
///
/// # Panics
///
/// Panics if the source bundle does not exist, any filesystem copy fails, or a
/// staged file cannot be read back or rewritten — every such failure is a test
/// setup error, not a condition under test.
#[allow(dead_code)]
pub fn stage_example_with(name: &str, layer_root: &Path, substitutions: &[(&str, &str)]) {
    stage_example(name, layer_root);
    let bundle = layer_root.join(PLUGINS_SUBDIR).join(name);
    apply_substitutions(&bundle, substitutions);
}

/// Rewrites placeholder tokens across every file in a staged bundle directory.
///
/// Recursively walks `bundle`, reading each file as UTF-8 and replacing every
/// occurrence of each `token` with its `replacement`. A file is rewritten only
/// when at least one token actually matched, so an unrelated file is left byte
/// for byte unchanged. Used by [`stage_example_with`] to specialize a staged
/// temp copy of a committed example.
///
/// # Parameters
///
/// - `bundle` — the staged bundle directory to walk.
/// - `substitutions` — the `(token, replacement)` pairs to apply.
///
/// # Panics
///
/// Panics if the directory cannot be walked or a file cannot be read or
/// written — a staging failure is a test setup error.
fn apply_substitutions(bundle: &Path, substitutions: &[(&str, &str)]) {
    let entries = std::fs::read_dir(bundle).unwrap_or_else(|error| {
        panic!(
            "staged bundle {} should be readable: {error}",
            bundle.display()
        )
    });
    for entry in entries {
        let entry = entry.expect("a staged directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            apply_substitutions(&path, substitutions);
            continue;
        }
        let original = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "staged file {} should be readable as UTF-8: {error}",
                path.display()
            )
        });
        let mut rewritten = original.clone();
        for (token, replacement) in substitutions {
            rewritten = rewritten.replace(token, replacement);
        }
        if rewritten != original {
            std::fs::write(&path, rewritten).unwrap_or_else(|error| {
                panic!(
                    "staged file {} should be rewritable: {error}",
                    path.display()
                )
            });
        }
    }
}

/// Recursively copies the directory tree at `source` to `destination`.
///
/// Creates `destination` and every nested directory, then copies each file
/// verbatim. Used by [`stage_example`] to lay a committed bundle into a temp
/// layer root.
///
/// # Parameters
///
/// - `source` — the directory tree to copy from.
/// - `destination` — the directory tree to create and copy into.
///
/// # Panics
///
/// Panics if any directory creation, directory read, or file copy fails.
fn copy_dir_recursive(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap_or_else(|error| {
        panic!(
            "staging directory {} should be created: {error}",
            destination.display(),
        )
    });
    let entries = std::fs::read_dir(source).unwrap_or_else(|error| {
        panic!(
            "example bundle {} should be readable: {error}",
            source.display()
        )
    });
    for entry in entries {
        let entry = entry.expect("a directory entry should be readable");
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to);
        } else {
            std::fs::copy(&from, &to).unwrap_or_else(|error| {
                panic!(
                    "example file {} should copy to {}: {error}",
                    from.display(),
                    to.display(),
                )
            });
        }
    }
}

/// Builds a real MCP server against an isolated temp working directory.
///
/// The temp `work_dir` keeps the server's bootstrap from walking the real
/// monorepo. `agent_mode` is `true` so the full in-process tool set — including
/// the unified `files` tool, an agent tool — is registered, which is what makes
/// the real tools reachable for exposure to a [`PluginHost`].
///
/// This is the canonical MCP-server bootstrap for the example tests, lifted
/// verbatim from `files_dispatch_e2e.rs::build_mcp_server` so every example
/// test shares one definition.
///
/// [`PluginHost`]: swissarmyhammer_plugin::PluginHost
///
/// # Parameters
///
/// - `work_dir` — the isolated temp directory the server bootstraps within.
///
/// # Returns
///
/// A fully bootstrapped [`McpServer`] with the real in-process tool registry.
///
/// # Panics
///
/// Panics if the MCP server bootstrap fails.
#[allow(dead_code)]
pub async fn build_mcp_server(work_dir: &Path) -> McpServer {
    McpServer::new_with_work_dir(PromptLibrary::new(), work_dir.to_path_buf(), None, true)
        .await
        .expect("MCP server bootstrap should succeed")
}

/// The module id the in-process `kanban` operation tool is exposed under.
///
/// A plugin addresses it with `register(name, { rust: "kanban" })`. It is the
/// kanban tool's own name — the id [`build_tool_modules`] keys the module by —
/// so the constant is purely documentary, naming what the example bundle's
/// `entry.ts` registers.
#[allow(dead_code)]
pub const KANBAN_MODULE_ID: &str = "kanban";

/// A handle to the in-process `kanban` operation tool exposed for a test.
///
/// [`expose_kanban_module`] returns one of these. It owns the live
/// [`ToolRegistry`] and [`ToolContext`] backing the exposed module — keeping
/// them alive for the whole test — and the `(id, server)` pair that
/// [`build_tool_modules`] produced. The handle does two things:
///
/// - [`expose_to`](Self::expose_to) hands the module to a [`PluginHost`] under
///   its id, the production exposure path a plugin's
///   `register("…", { rust: "kanban" })` then activates;
/// - [`call`](Self::call) drives the same module directly from the test, so a
///   test can seed the board (`init board`) before load and read it back
///   (`list tasks`) after — observing the one effect a passing run produces.
///
/// Both the host-exposed module and the test's direct calls run against the
/// **same** registry and context, so they see the same `.kanban` board: what
/// the plugin writes through the host, the test reads back here.
pub struct ExposedKanban {
    /// The registry holding the kanban tools; kept alive so the wrapped
    /// module's live-tool resolution keeps succeeding.
    _registry: Arc<RwLock<ToolRegistry>>,
    /// The context every kanban execution is threaded through — pinned to the
    /// temp board root; kept alive for the module's lifetime.
    _context: Arc<ToolContext>,
    /// The module id (the kanban tool's name) the module is exposed under.
    module_id: String,
    /// The platform server wrapping the kanban tool — exposed to a host or
    /// invoked directly.
    module: Arc<dyn PluginMcpServer>,
}

#[allow(dead_code)]
impl ExposedKanban {
    /// Exposes the wrapped `kanban` module to `host` under its module id.
    ///
    /// This is the production exposure path: the module is recorded in the
    /// host's available-modules table but stays inert until a plugin activates
    /// it with `register("board", { rust: "kanban" })`.
    ///
    /// # Parameters
    ///
    /// - `host` — the plugin host to expose the `kanban` module into.
    ///
    /// # Errors
    ///
    /// Returns the host error when `expose_rust_module` rejects the id — in
    /// practice, an id already exposed.
    pub async fn expose_to(&self, host: &PluginHost) -> swissarmyhammer_plugin::Result<()> {
        host.expose_rust_module(self.module_id.clone(), Arc::clone(&self.module))
            .await
    }

    /// Invokes the wrapped `kanban` tool directly with an arguments object.
    ///
    /// Drives the very same module the host exposes, bypassing the plugin
    /// pipeline — used by a test to seed the board before a plugin loads and to
    /// read it back afterward. `args` is a kanban arguments object carrying an
    /// `op` selector, e.g. `{ "op": "init board", "name": "…" }`.
    ///
    /// # Parameters
    ///
    /// - `args` — the kanban tool arguments, including the `op` selector.
    ///
    /// # Returns
    ///
    /// The tool's `CallToolResult` serialized to a [`Value`], the shape an MCP
    /// `tools/call` response carries on the wire.
    ///
    /// # Errors
    ///
    /// Returns the platform error when the kanban tool's execution fails.
    pub async fn call(&self, args: Value) -> swissarmyhammer_plugin::Result<Value> {
        self.module
            .invoke(CallerId::HostInternal, &self.module_id, args)
            .await
    }
}

/// Exposes the in-process `kanban` operation tool, rooted at a temp board.
///
/// Builds a minimal [`ToolRegistry`] holding only the kanban tools and pairs it
/// with a [`ToolContext`] whose `working_dir` is `board_root` — so the kanban
/// tool resolves its board at `<board_root>/.kanban`. The tool is wrapped via
/// the `swissarmyhammer-tools` [`build_tool_modules`] adapter — the same path
/// `McpServer::expose_tools_to_plugin_host` uses — into a platform server.
///
/// This mirrors the production wiring in the kanban desktop app
/// (`apps/kanban-app/src/plugins.rs::expose_kanban_module`): the same
/// `register_kanban_tools` / `build_tool_modules` / `ToolContext` triple, so
/// the example test drives a genuinely production-shaped `kanban` module.
///
/// The returned [`ExposedKanban`] owns the registry and context; keep it alive
/// for the whole test. Call [`ExposedKanban::expose_to`] to hand the module to
/// a [`PluginHost`], and [`ExposedKanban::call`] to drive it directly.
///
/// Async because [`build_tool_modules`] enumerates the registry under an async
/// lock — the example tests are already `#[tokio::test]`, so they `.await` it.
///
/// # Parameters
///
/// - `board_root` — the temp directory the kanban tool resolves its `.kanban`
///   board against.
///
/// # Returns
///
/// An [`ExposedKanban`] handle to the live `kanban` module.
///
/// # Panics
///
/// Panics if the kanban tool registry does not yield exactly one module — the
/// registry holds only the single `kanban` tool, so anything else is a wiring
/// error rather than a condition under test.
#[allow(dead_code)]
pub async fn expose_kanban_module(board_root: &Path) -> ExposedKanban {
    let mut registry = ToolRegistry::new();
    register_kanban_tools(&mut registry);
    let registry = Arc::new(RwLock::new(registry));

    // The kanban tool only needs `working_dir` (to locate its `.kanban` board)
    // and the registry handle; git is not used, so `git_ops` is `None`.
    let git_ops = Arc::new(TokioMutex::new(None::<GitOperations>));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());
    let context = ToolContext::new(tool_handlers, git_ops, agent_config)
        .with_tool_registry(Arc::clone(&registry))
        .with_working_dir(board_root.to_path_buf());
    let context = Arc::new(context);

    let modules = build_tool_modules(Arc::clone(&registry), Arc::clone(&context)).await;

    let mut modules = modules.into_iter();
    let (module_id, module) = modules
        .next()
        .expect("the kanban registry must yield its one tool module");
    assert!(
        modules.next().is_none(),
        "the kanban-only registry must expose exactly one module",
    );

    ExposedKanban {
        _registry: registry,
        _context: context,
        module_id,
        module,
    }
}

/// Extracts the task titles from a `kanban` `list tasks` result.
///
/// A `kanban` `list tasks` call returns a `CallToolResult` shape — a JSON
/// object with a `content` array whose first entry's `text` is the listing
/// JSON. The listing is itself an object `{ "tasks": [...], "count": N }`;
/// each task in the `tasks` array carries a `title` string. This walks that
/// shape and returns the titles in board order.
///
/// # Parameters
///
/// - `result` — a `list tasks` result as returned by [`ExposedKanban::call`].
///
/// # Returns
///
/// The `title` of every task in the listed result.
///
/// # Panics
///
/// Panics if `result` is not the expected `CallToolResult`/task-listing shape
/// — a malformed result is a test wiring error, not a condition under test.
#[allow(dead_code)]
pub fn task_titles(result: &Value) -> Vec<String> {
    let text = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|entry| entry.get("text"))
        .and_then(Value::as_str)
        .expect("a `list tasks` result must carry text content");
    let listing: Value = serde_json::from_str(text).expect("`list tasks` content must be JSON");
    listing
        .get("tasks")
        .and_then(Value::as_array)
        .expect("`list tasks` content must carry a `tasks` array")
        .iter()
        .map(|task| {
            task.get("title")
                .and_then(Value::as_str)
                .expect("every listed task must carry a title")
                .to_string()
        })
        .collect()
}
