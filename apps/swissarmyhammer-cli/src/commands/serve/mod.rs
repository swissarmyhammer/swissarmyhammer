//! Serve command implementation
//!
//! Starts the SwissArmyHammer MCP (Model Context Protocol) server for AI tool integration.
//!
//! This module provides the serve command which starts an MCP server that exposes
//! SwissArmyHammer tools and capabilities through the Model Context Protocol.
//! This enables integration with AI applications like Claude Code.
//!
//! # Features
//!
//! - Tool integration through MCP protocol
//! - Stdio transport for client communication
//! - Graceful shutdown handling
//! - Comprehensive logging and error handling
//! - Integration with SwissArmyHammer tool ecosystem

use crate::context::CliContext;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};
use std::sync::Arc;
use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_templating::TemplateLibrary;
use swissarmyhammer_tools::mcp::unified_server::McpServerHandle;

pub mod display;

/// Wire the live `review` agent + embedder factories into a freshly-started MCP
/// server.
///
/// The server registers the `review` tool with no agent factory (so its
/// pipeline ops error until wired). This is the cycle-free injection point: the
/// CLI may depend on `swissarmyhammer-agent`, which `swissarmyhammer-tools`
/// cannot. We build the production `review_agent_factory` from the server's
/// own resolved `ModelConfig` and register the configured tool into the shared
/// registry the serving task reads on every `call_tool`. A pinned
/// `review.concurrency` is honored when set; the platform embedder default is
/// kept (`None`).
///
/// A no-op when the handle exposes no server instance (it always does for the
/// stdio/HTTP serve paths).
///
/// `review_override` is the resolved review-specific `ModelConfig` (from
/// [`review_model_config`]); when `Some`, the review pool's agent factory is
/// built from it instead of the server's global `agent_config`. When `None`
/// (review model unset or unresolvable) the global default is used unchanged.
async fn wire_review_factories(
    handle: &McpServerHandle,
    review_override: Option<Arc<ModelConfig>>,
    concurrency: Option<usize>,
) {
    use swissarmyhammer_agent::review_agent_factory;

    let Some(server) = handle.server() else {
        return;
    };
    let model_config = review_override.unwrap_or_else(|| server.tool_context.agent_config.clone());
    let factory = review_agent_factory(model_config);
    server
        .set_review_factories(factory, None, concurrency)
        .await;
}

/// Resolve the review-specific model override from the `.sah` config files.
///
/// Model SELECTION reads the config files via the canonical resolver
/// ([`ModelManager::resolve_review_agent_name`] over [`ModelPaths::sah`]) — NOT
/// the template context. The template `model` / `review.model` variables are a
/// prompt-rendering concern (`set_model_variable` injects a literal `"claude"`
/// default for `{{ model }}` Liquid expansion) and must never drive agent
/// selection; consuming them here is exactly what defeated the
/// `claude-code-haiku` review fallback.
///
/// The shared review-scope precedence ([`ModelManager::review_agent_name_from`])
/// applies, reading from config files:
/// - explicit `review.model` wins (review only);
/// - else an explicit overall `model:` drives review too ("if I set an overall
///   model I mean it");
/// - else the baked-in `claude-code-haiku` ([`REVIEW_DEFAULT_AGENT`]) factory
///   default is used. The config-file accessors return `None` when unset, so a
///   fully unconfigured scope falls through to `claude-code-haiku`.
///
/// The resolved name is loaded via [`ModelManager::find_agent_by_name`] +
/// [`parse_model_config`]. Returns `None` only when that name cannot be
/// resolved or parsed (a warning is logged); in that case
/// [`wire_review_factories`] falls back to the global `agent_config`.
///
/// Deliberate downgrade of the *unresolvable* case: a fully unconfigured scope
/// resolves to `claude-code-haiku` (the precedence default), but a
/// *misconfigured* `review.model` (a name that does not resolve) returns `None`
/// → the global `agent_config` (plain `claude-code`, no `--model`), NOT the haiku
/// review default. This is intentional — an explicit-but-broken `review.model`
/// should not be silently "fixed" to haiku; it warns and falls back to the
/// server's overall agent. (This is why we resolve the *name* via
/// [`ModelManager::resolve_review_agent_name`] and parse it here, rather than
/// calling [`ModelManager::resolve_review_agent_config`], which would itself
/// re-resolve to the config-file global default instead of the server's live
/// `agent_config`.) Covered by
/// `test_review_model_config_unknown_returns_none_for_global_fallback` and
/// `test_review_model_config_defaults_to_haiku_when_unset`.
fn review_model_config(cli_context: &CliContext) -> Option<Arc<ModelConfig>> {
    use swissarmyhammer_config::model::{parse_model_config, ModelManager, ModelPaths};

    // `cli_context` is unused for selection on purpose: review-model selection
    // is a CONFIG-FILE decision, deliberately independent of the prompt-rendering
    // template variables this context carries.
    let _ = cli_context;

    let model_name = match ModelManager::resolve_review_agent_name(&ModelPaths::sah()) {
        Ok(name) => name,
        Err(e) => {
            tracing::warn!(
                "could not resolve review model name from config ({}); falling back to the global agent config",
                e
            );
            return None;
        }
    };

    match ModelManager::find_agent_by_name(&model_name)
        .and_then(|info| Ok(parse_model_config(&info.content)?))
    {
        Ok(config) => {
            // Decision-point record so the tier a review run uses is provable in
            // the `.sah` logs.
            tracing::info!(
                "review scope → {} ({:?})",
                model_name,
                config.executor_type()
            );
            Some(Arc::new(config))
        }
        Err(e) => {
            tracing::warn!(
                "review model '{}' could not be resolved ({}); falling back to the global agent config",
                model_name,
                e
            );
            None
        }
    }
}

/// Read the `review.concurrency` config override (a positive integer pinning the
/// review pool worker count). Returns `None` when unset, non-numeric, or `0`.
fn review_concurrency(cli_context: &CliContext) -> Option<usize> {
    cli_context
        .template_context
        .get("review.concurrency")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .filter(|n| *n > 0)
}

/// Help text for the serve command
#[cfg(test)]
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the serve command
///
/// Starts the MCP server with stdio or HTTP transport based on subcommands.
/// The server runs in blocking mode until the client disconnects or an error occurs.
///
/// # Arguments
///
/// * `matches` - Command line arguments for serve command and subcommands
/// * `cli_context` - CLI context with configuration and global arguments
///
/// # Returns
///
/// Returns an exit code:
/// - 0: Server started and stopped successfully
/// - 1: Server encountered warnings or stopped unexpectedly
/// - 2: Server failed to start or encountered critical errors
pub async fn handle_command(matches: &clap::ArgMatches, cli_context: &CliContext) -> i32 {
    // Extract global --model flag from root matches
    let model_override = cli_context
        .matches
        .get_one::<String>("model")
        .map(|s| s.to_string());

    // Check for HTTP subcommand
    match matches.subcommand() {
        Some(("http", http_matches)) => {
            handle_http_serve(http_matches, cli_context, model_override).await
        }
        None => {
            // Default to stdio mode (existing behavior)
            handle_stdio_serve(cli_context, model_override).await
        }
        Some((unknown, _)) => {
            eprintln!("Unknown serve subcommand: {}", unknown);
            EXIT_ERROR
        }
    }
}

/// Handle HTTP serve mode
async fn handle_http_serve(
    matches: &clap::ArgMatches,
    cli_context: &CliContext,
    model_override: Option<String>,
) -> i32 {
    let server_handle = match initialize_http_server(matches, cli_context, model_override).await {
        Ok(handle) => handle,
        Err(exit_code) => return exit_code,
    };

    wire_review_factories(
        &server_handle,
        review_model_config(cli_context),
        review_concurrency(cli_context),
    )
    .await;

    manage_http_server_lifecycle(cli_context, server_handle).await
}

/// Initialize HTTP server and return handle or error exit code
async fn initialize_http_server(
    matches: &clap::ArgMatches,
    cli_context: &CliContext,
    model_override: Option<String>,
) -> Result<McpServerHandle, i32> {
    use swissarmyhammer_tools::mcp::{start_mcp_server, McpServerMode};

    // Prove which build this process is running. A running `sah serve`
    // keeps its launch-time code even after the on-disk binary is rebuilt,
    // so record the baked-in git SHA in this process's own log.
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        git_sha = swissarmyhammer_common::build_info::GIT_SHA,
        "sah serve starting"
    );

    let port: u16 = matches.get_one::<u16>("port").copied().unwrap_or(8000);
    let host = matches
        .get_one::<String>("host")
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1");

    let bind_addr = format!("{}:{}", host, port);

    if host != "127.0.0.1" {
        eprintln!(
            "Warning: Custom host '{}' not yet supported by unified server, using 127.0.0.1",
            host
        );
    }

    display_server_status(
        cli_context,
        "HTTP",
        "Starting",
        &bind_addr,
        Some(port),
        0,
        &format!("Starting SwissArmyHammer MCP server on {}", bind_addr),
    );

    println!(
        "Starting SwissArmyHammer MCP server on 127.0.0.1:{}",
        if port == 0 {
            "random port".to_string()
        } else {
            port.to_string()
        }
    );

    let mode = McpServerMode::Http { port: Some(port) };
    let server_handle = start_mcp_server(mode, None, model_override, None)
        .await
        .map_err(|e| {
            tracing::error!("Failed to start HTTP MCP server: {}", e);
            display_server_status(
                cli_context,
                "HTTP",
                "Error",
                &bind_addr,
                Some(port),
                0,
                &format!("Failed to start HTTP MCP server: {}", e),
            );
            EXIT_ERROR
        })?;

    display_http_server_running_status(cli_context, &server_handle, port);
    Ok(server_handle)
}

/// Display running status for HTTP server
fn display_http_server_running_status(
    cli_context: &CliContext,
    handle: &McpServerHandle,
    requested_port: u16,
) {
    let actual_port = handle.port().unwrap_or(requested_port);
    let running_message =
        format_http_server_running_message(handle.url(), requested_port, actual_port);

    display_server_status(
        cli_context,
        "HTTP",
        "Running",
        handle.url(),
        Some(actual_port),
        0,
        &running_message,
    );
}

/// Format the running message for HTTP server
fn format_http_server_running_message(url: &str, requested_port: u16, actual_port: u16) -> String {
    if requested_port == 0 {
        format!(
            "✓ MCP HTTP server running on {} (bound to random port: {}). 💡 Use Ctrl+C to stop.",
            url, actual_port
        )
    } else {
        format!(
            "✓ MCP HTTP server running on {}. 💡 Use Ctrl+C to stop.",
            url
        )
    }
}

/// Manage HTTP server lifecycle including shutdown
async fn manage_http_server_lifecycle(
    cli_context: &CliContext,
    mut server_handle: McpServerHandle,
) -> i32 {
    use crate::signal_handler::wait_for_shutdown;

    wait_for_shutdown().await;

    display_server_status(
        cli_context,
        "HTTP",
        "Stopping",
        server_handle.url(),
        server_handle.port(),
        0,
        "🛑 Shutting down server...",
    );

    if let Err(e) = server_handle.shutdown().await {
        tracing::error!("Failed to shutdown server gracefully: {}", e);
        display_server_status(
            cli_context,
            "HTTP",
            "Error",
            server_handle.url(),
            server_handle.port(),
            0,
            &format!("Warning: Server shutdown error: {}", e),
        );
        return EXIT_WARNING;
    }

    if let Err(e) = server_handle.wait_for_completion().await {
        tracing::error!("Error waiting for server task completion: {}", e);
        display_server_status(
            cli_context,
            "HTTP",
            "Error",
            "-",
            None,
            0,
            &format!("Warning: Server task completion error: {}", e),
        );
        return EXIT_WARNING;
    }

    display_server_status(
        cli_context,
        "HTTP",
        "Stopped",
        "-",
        None,
        0,
        "✓ Server stopped",
    );

    EXIT_SUCCESS
}

/// Handle stdio serve mode (existing behavior)
async fn handle_stdio_serve(cli_context: &CliContext, model_override: Option<String>) -> i32 {
    let (library, prompt_count) = match initialize_prompt_library(cli_context) {
        Ok(result) => result,
        Err(exit_code) => return exit_code,
    };

    let server_handle =
        match start_stdio_server(cli_context, library, prompt_count, model_override).await {
            Ok(handle) => handle,
            Err(exit_code) => return exit_code,
        };

    wire_review_factories(
        &server_handle,
        review_model_config(cli_context),
        review_concurrency(cli_context),
    )
    .await;

    handle_stdio_server_shutdown(server_handle).await
}

/// Initialize prompt library for stdio mode
fn initialize_prompt_library(cli_context: &CliContext) -> Result<(TemplateLibrary, usize), i32> {
    tracing::debug!("Starting unified MCP server in stdio mode");

    let library = cli_context.get_prompt_library().map_err(|e| {
        tracing::error!("Failed to load prompts: {}", e);
        display_server_status(
            cli_context,
            "Stdio",
            "Error",
            "stdio",
            None,
            0,
            &format!("Failed to load prompts: {}", e),
        );
        EXIT_ERROR
    })?;

    let prompt_count = library.list().map(|p| p.len()).unwrap_or(0);
    tracing::debug!("Loaded {} prompts for MCP server", prompt_count);

    Ok((library, prompt_count))
}

/// Start stdio server and return handle or error exit code
async fn start_stdio_server(
    cli_context: &CliContext,
    library: TemplateLibrary,
    prompt_count: usize,
    model_override: Option<String>,
) -> Result<McpServerHandle, i32> {
    use swissarmyhammer_tools::mcp::{start_mcp_server, McpServerMode};

    // Prove which build this process is running. A running `sah serve`
    // keeps its launch-time code even after the on-disk binary is rebuilt,
    // so record the baked-in git SHA in this process's own log.
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        git_sha = swissarmyhammer_common::build_info::GIT_SHA,
        "sah serve starting"
    );

    if cli_context.verbose {
        display_server_status(
            cli_context,
            "Stdio",
            "Starting",
            "stdio",
            None,
            prompt_count,
            "Starting MCP server in stdio mode",
        );
    }

    let mode = McpServerMode::Stdio;
    start_mcp_server(mode, Some(library), model_override, None)
        .await
        .map_err(|e| {
            tracing::error!("Failed to start unified stdio MCP server: {}", e);
            eprintln!("Failed to start unified stdio MCP server: {}", e);
            EXIT_ERROR
        })
}

/// Handle stdio server shutdown and completion
async fn handle_stdio_server_shutdown(mut server_handle: McpServerHandle) -> i32 {
    wait_for_stdio_server_termination(&mut server_handle).await;
    finalize_stdio_server_shutdown(server_handle).await
}

/// Wait for server termination via signal or natural completion
async fn wait_for_stdio_server_termination(server_handle: &mut McpServerHandle) {
    use crate::signal_handler::wait_for_shutdown;

    let mut completion_rx = server_handle.take_completion_rx();

    tokio::select! {
        _ = wait_for_shutdown() => {
            handle_shutdown_signal(server_handle).await;
        }
        _ = wait_for_natural_completion(completion_rx.as_mut()) => {
            tracing::info!("Server completed naturally (EOF on stdin)");
        }
    }
}

/// Handle shutdown signal for stdio server
async fn handle_shutdown_signal(server_handle: &mut McpServerHandle) {
    tracing::info!("Received shutdown signal (SIGTERM/CTRL+C)");
    if let Err(e) = server_handle.shutdown().await {
        tracing::warn!("Error sending shutdown signal: {}", e);
    }
}

/// Wait for natural completion of server
async fn wait_for_natural_completion(
    completion_rx: Option<&mut tokio::sync::oneshot::Receiver<()>>,
) {
    if let Some(rx) = completion_rx {
        let _ = rx.await;
    } else {
        std::future::pending::<()>().await
    }
}

/// Finalize server shutdown and return exit code
async fn finalize_stdio_server_shutdown(mut server_handle: McpServerHandle) -> i32 {
    if let Err(e) = server_handle.wait_for_completion().await {
        tracing::error!("Error waiting for server task completion: {}", e);
        return EXIT_ERROR;
    }

    tracing::info!("MCP stdio server completed successfully");
    EXIT_SUCCESS
}

/// Helper function to display server status based on verbose flag
fn display_server_status(
    cli_context: &CliContext,
    server_type: &str,
    status: &str,
    address: &str,
    port: Option<u16>,
    prompt_count: usize,
    message: &str,
) {
    if !cli_context.verbose {
        display_basic_server_status(cli_context, server_type, status, address, message);
        return;
    }

    display_verbose_server_status(
        cli_context,
        server_type,
        status,
        address,
        port,
        prompt_count,
        message,
    );
}

/// Display basic server status
fn display_basic_server_status(
    cli_context: &CliContext,
    server_type: &str,
    status: &str,
    address: &str,
    message: &str,
) {
    let basic_status = vec![display::ServerStatus::new(
        server_type.to_string(),
        status.to_string(),
        address.to_string(),
        message.to_string(),
    )];

    if let Err(e) = cli_context.display(&basic_status) {
        eprintln!("Failed to display status: {}", e);
    }
}

/// Display verbose server status with additional details
fn display_verbose_server_status(
    cli_context: &CliContext,
    server_type: &str,
    status: &str,
    address: &str,
    port: Option<u16>,
    prompt_count: usize,
    message: &str,
) {
    let health_url = port.map(|p| {
        format!(
            "http://{}:{}/health",
            address.split(':').next().unwrap_or("127.0.0.1"),
            p
        )
    });

    let verbose_status = vec![display::VerboseServerStatus::new(
        server_type.to_string(),
        status.to_string(),
        address.to_string(),
        port,
        health_url,
        prompt_count,
        message.to_string(),
    )];

    if let Err(e) = cli_context.display(&verbose_status) {
        eprintln!("Failed to display status: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, Command};
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
    use tracing_test::traced_test;

    #[test]
    fn test_description_content() {
        assert!(DESCRIPTION.contains("MCP server"));
        assert!(DESCRIPTION.contains("Bridge AI"));
        assert!(
            DESCRIPTION.len() > 100,
            "Description should be comprehensive"
        );
    }

    /// Build a production-shaped `CliContext` and write the given `.sah/sah.yaml`
    /// config content (when `Some`), with HOME/CWD isolated so config resolution
    /// does not touch the host filesystem.
    ///
    /// Model SELECTION for the review scope reads the config FILES, so configured
    /// scenarios are expressed by writing real `.sah` config (mirroring the
    /// `model.rs` resolver tests) — not by injecting template variables.
    ///
    /// Crucially, the template context is produced via
    /// [`swissarmyhammer_config::TemplateContext::set_default_variables`], exactly
    /// like production (`load_for_cli`). That injects the defaulted prompt
    /// variable `model = "claude"` whenever `model` is unset — the very value that
    /// used to leak into agent selection. A correct `review_model_config` must
    /// ignore it and resolve from config files only.
    async fn cli_context_with_sah_config(
        config_yaml: Option<&str>,
    ) -> (
        crate::context::CliContext,
        IsolatedTestEnvironment,
        CurrentDirGuard,
    ) {
        use crate::cli::OutputFormat;
        use crate::context::CliContext;
        use swissarmyhammer_config::model::{ModelManager, ModelPaths};

        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        if let Some(yaml) = config_yaml {
            let config_path = ModelManager::ensure_config_structure(&ModelPaths::sah())
                .expect("config structure");
            std::fs::write(&config_path, yaml).expect("write .sah config");
        }

        let app = Command::new("test").arg(Arg::new("test").long("test"));
        let matches = app.try_get_matches_from(vec!["test"]).unwrap();

        // Reproduce the production template-context shape: `set_default_variables`
        // injects `model = "claude"` (a PROMPT-rendering default) when unset.
        let mut template_context = swissarmyhammer_config::TemplateContext::new();
        template_context.set_default_variables();

        let cli_context = CliContext::new(
            template_context,
            OutputFormat::Table,
            None,
            false,
            false,
            false,
            matches,
        )
        .await
        .expect("Failed to create CliContext");

        (cli_context, env, cwd)
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_review_model_config_resolves_configured_llama_model() {
        use swissarmyhammer_config::model::ModelExecutorType;

        let (cli_context, _env, _cwd) =
            cli_context_with_sah_config(Some("review:\n  model: qwen-0.6b-test\n")).await;

        let resolved =
            review_model_config(&cli_context).expect("a configured review.model must resolve");
        assert_eq!(
            resolved.executor_type(),
            ModelExecutorType::LlamaAgent,
            "qwen-0.6b-test must resolve to the llama-agent executor"
        );
    }

    // Reproduces production: an unconfigured project, with the defaulted prompt
    // variable `model = "claude"` present in the template context (via
    // `set_default_variables`), must STILL resolve the review scope to the
    // baked-in `claude-code-haiku` (`--model haiku`). The defaulted `model`
    // template var must not leak into selection.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_review_model_config_defaults_to_haiku_when_unset() {
        use swissarmyhammer_config::model::{ModelExecutorConfig, ModelExecutorType};

        let (cli_context, _env, _cwd) = cli_context_with_sah_config(None).await;

        let resolved = review_model_config(&cli_context)
            .expect("an unset review.model must resolve to the baked-in claude-code-haiku default");
        assert_eq!(resolved.executor_type(), ModelExecutorType::ClaudeCode);
        match resolved.executor() {
            ModelExecutorConfig::ClaudeCode(claude_config) => {
                assert_eq!(
                    claude_config.args,
                    vec!["--model".to_string(), "haiku".to_string()],
                    "unset review.model must default to claude-code-haiku (--model haiku)"
                );
            }
            _ => panic!("Should be Claude Code config"),
        }
    }

    // Regression guard for the root cause: the prompt-rendering `model` template
    // variable must NEVER be consulted for review-model SELECTION. Setting it to a
    // bogus value (here, an unresolvable model name) must not change the resolved
    // review ModelConfig — it must remain the config-file-driven haiku default.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_review_model_config_ignores_template_model_var() {
        use serde_json::json;
        use swissarmyhammer_config::model::{ModelExecutorConfig, ModelExecutorType};

        let (mut cli_context, _env, _cwd) = cli_context_with_sah_config(None).await;
        // Overwrite the template `model` var with a bogus value. If selection
        // consulted template vars, this would make resolution fail (None) instead
        // of yielding the haiku default.
        cli_context
            .template_context
            .set("model".to_string(), json!("definitely-not-a-real-model"));

        let resolved = review_model_config(&cli_context).expect(
            "a bogus template `model` var must not affect selection; haiku default must hold",
        );
        assert_eq!(resolved.executor_type(), ModelExecutorType::ClaudeCode);
        match resolved.executor() {
            ModelExecutorConfig::ClaudeCode(claude_config) => {
                assert_eq!(
                    claude_config.args,
                    vec!["--model".to_string(), "haiku".to_string()],
                    "template `model` var must not be consulted for selection"
                );
            }
            _ => panic!("Should be Claude Code config"),
        }
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_review_model_config_unknown_returns_none_for_global_fallback() {
        // An unresolvable review.model (and no overall model) cannot be honored,
        // so this returns None and `wire_review_factories` falls back to the
        // global agent_config — it must not panic.
        let (cli_context, _env, _cwd) =
            cli_context_with_sah_config(Some("review:\n  model: definitely-not-a-real-model\n"))
                .await;

        assert!(
            review_model_config(&cli_context).is_none(),
            "an unresolvable review.model must return None (global fallback), not the haiku default"
        );
    }

    // When an overall model: is set but review.model is not, the review scope
    // follows the overall default rather than the baked-in claude-code-haiku.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_review_model_config_inherits_overall_default() {
        use swissarmyhammer_config::model::ModelExecutorType;

        let (cli_context, _env, _cwd) =
            cli_context_with_sah_config(Some("model: qwen-0.6b-test\n")).await;

        let resolved = review_model_config(&cli_context)
            .expect("review should inherit the explicit overall model");
        assert_eq!(
            resolved.executor_type(),
            ModelExecutorType::LlamaAgent,
            "an overall model: must drive the review scope when review.model is unset"
        );
    }

    // An explicit review.model overrides the overall model: for review only.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_review_model_config_review_overrides_overall() {
        use swissarmyhammer_config::model::{ModelExecutorConfig, ModelExecutorType};

        let (cli_context, _env, _cwd) = cli_context_with_sah_config(Some(
            "model: qwen-0.6b-test\nreview:\n  model: claude-code\n",
        ))
        .await;

        let resolved =
            review_model_config(&cli_context).expect("explicit review.model must resolve");
        assert_eq!(resolved.executor_type(), ModelExecutorType::ClaudeCode);
        match resolved.executor() {
            ModelExecutorConfig::ClaudeCode(claude_config) => {
                assert!(
                    claude_config.args.is_empty(),
                    "explicit review.model: claude-code must win over the overall model"
                );
            }
            _ => panic!("Should be Claude Code config"),
        }
    }

    // Decision-point logging: resolving an explicit `claude-code-haiku` review
    // model must record the resolved name + executor in the logs, so the tier a
    // review run used is provable in the `.sah` logs.
    #[tokio::test]
    #[traced_test]
    #[serial_test::serial(cwd)]
    async fn test_review_model_config_logs_resolved_name_and_executor() {
        let (cli_context, _env, _cwd) =
            cli_context_with_sah_config(Some("review:\n  model: claude-code-haiku\n")).await;

        review_model_config(&cli_context).expect("claude-code-haiku must resolve");

        assert!(
            logs_contain("review scope → claude-code-haiku"),
            "resolving a review model must log the decision point with the resolved name"
        );
        assert!(
            logs_contain("ClaudeCode"),
            "resolving a review model must log the executor type"
        );
    }

    // When the review model is unresolvable, the global-fallback path must be
    // logged (not silent), so operators can see why no review tier was applied.
    #[tokio::test]
    #[traced_test]
    #[serial_test::serial(cwd)]
    async fn test_review_model_config_logs_global_fallback() {
        let (cli_context, _env, _cwd) =
            cli_context_with_sah_config(Some("review:\n  model: definitely-not-a-real-model\n"))
                .await;

        assert!(
            review_model_config(&cli_context).is_none(),
            "an unresolvable review.model must return None"
        );
        assert!(
            logs_contain("global agent config"),
            "the global-fallback case must be logged"
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_handle_command_signature() {
        use crate::cli::OutputFormat;
        use crate::context::CliContext;

        // Isolate HOME + CWD — `CliContext::new()` calls into the same
        // `build_async()` codepath that creates `.sah/` at cwd as a side effect
        // of `get_swissarmyhammer_dir()`. Without isolation, this test leaks a
        // `.sah/` skeleton into the host crate directory.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        // This test just verifies that the function signature matches expected pattern
        let app = Command::new("test").arg(Arg::new("test").long("test"));
        let matches = app.try_get_matches_from(vec!["test"]).unwrap();

        // Create a test CliContext
        let template_context = swissarmyhammer_config::TemplateContext::new();
        let cli_context = CliContext::new(
            template_context,
            OutputFormat::Table,
            None,
            false,
            false,
            false,
            matches.clone(),
        )
        .await
        .expect("Failed to create CliContext");

        // We can verify the signature compiles and matches expected pattern
        let _result: std::pin::Pin<Box<dyn std::future::Future<Output = i32>>> =
            Box::pin(handle_command(&matches, &cli_context));
    }
}
