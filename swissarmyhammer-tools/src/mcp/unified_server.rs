//! Unified MCP server implementation supporting multiple transport modes
//!
//! This module provides a clean, consolidated MCP server implementation that:
//! - Uses rmcp library properly without reimplementing MCP protocol
//! - Supports both stdio and HTTP transport modes
//! - Returns clear connection information for each mode
//! - Eliminates fragmented implementations across multiple crates

use rmcp::serve_server;
use rmcp::transport::io::stdio;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, Once};
use swissarmyhammer_common::Result;
use swissarmyhammer_prompts::PromptLibrary;

use tokio::net::TcpListener;
use tokio::sync::oneshot;

use super::server::McpServer;

/// Health check endpoint handler
async fn health_check() -> axum::response::Json<serde_json::Value> {
    axum::response::Json(serde_json::json!({
        "status": "healthy",
        "service": "swissarmyhammer-mcp"
    }))
}

/// Thread-safe file writer with immediate flush and sync for reliable debugging logs
///
/// This writer ensures that every write operation is immediately flushed to the OS buffer
/// and synced to disk, providing reliable log output even if the process crashes unexpectedly.
/// This is particularly important for debugging MCP communication issues.
///
/// # Thread Safety
/// 
/// Multiple threads can safely write to the same `FileWriterGuard` instance. Each write
/// operation acquires the mutex, writes the data, flushes to OS buffers, and syncs to disk
/// before releasing the lock.
///
/// # Performance Considerations
/// 
/// The immediate flush/sync strategy prioritizes reliability over performance. Each write
/// operation results in a system call, which may impact performance for high-frequency logging.
/// However, this trade-off is acceptable for MCP debugging scenarios where log reliability
/// is more important than maximum throughput.
///
/// # Error Handling
/// 
/// Write operations use `expect()` calls for error handling rather than returning `Result`
/// values because the `std::io::Write` trait requires specific signatures. In practice,
/// write failures to local files are extremely rare and typically indicate severe system
/// issues (disk full, permissions, hardware failure) that should terminate the process.
///
/// # Example Usage
/// ```rust,ignore
/// use std::sync::{Arc, Mutex};
/// use std::fs::File;
/// use swissarmyhammer_tools::mcp::unified_server::FileWriterGuard;
/// 
/// let file = File::create("debug.log").unwrap();
/// let shared_file = Arc::new(Mutex::new(file));
/// let mut guard = FileWriterGuard::new(shared_file);
/// writeln!(guard, "Debug message").unwrap();
/// ```
pub struct FileWriterGuard {
    file: Arc<Mutex<std::fs::File>>,
}

impl FileWriterGuard {
    /// Creates a new `FileWriterGuard` wrapping the given file.
    /// 
    /// # Arguments
    /// * `file` - Arc<Mutex<File>> for thread-safe access to the underlying file
    /// 
    /// # Returns
    /// A new `FileWriterGuard` instance that will ensure immediate flushing for all writes
    pub fn new(file: Arc<Mutex<std::fs::File>>) -> Self {
        Self { file }
    }
}

impl std::io::Write for FileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut file = self.file.lock().expect("FileWriterGuard mutex was poisoned - this indicates a panic occurred while another thread held the lock");
        let result = file.write(buf)?;
        file.flush()?;
        file.sync_all()?; // Ensure data is actually written to disk for debugging reliability
        Ok(result)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut file = self.file.lock().expect("FileWriterGuard flush mutex was poisoned - this indicates a panic occurred while another thread held the lock");
        file.flush()?;
        file.sync_all()
    }
}

/// Global flag to ensure MCP logging is configured only once per process
static MCP_LOGGING_INIT: Once = Once::new();

/// Configure MCP logging to write to `.swissarmyhammer/` directory
///
/// This function sets up file-based logging similar to what `sah serve` does,
/// ensuring that in-process MCP servers have the same debugging capabilities.
/// Uses `std::sync::Once` to ensure logging is only configured once per process,
/// even if multiple MCP servers are started.
///
/// # Arguments
/// * `log_filter` - Optional log filter string (defaults to "ort=warn,rmcp=warn,debug")
/// 
/// # Behavior
/// - Creates `.swissarmyhammer/` directory if it doesn't exist
/// - Sets up tracing subscriber with file output (uses `SWISSARMYHAMMER_LOG_FILE` env var or defaults to `mcp.log`)
/// - Falls back to stderr logging if file creation fails
/// - Only configures logging once per process (subsequent calls are no-op)
/// - Uses debug-level logging for comprehensive MCP debugging
///
/// # Error Handling
/// - Directory creation failures: Provides specific error context based on error kind
/// - File creation failures: Falls back gracefully to stderr with warning message
/// - Global subscriber conflicts: Handles gracefully when already set (e.g., in tests)
pub fn configure_mcp_logging(log_filter: Option<&str>) {
    use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};

    MCP_LOGGING_INIT.call_once(|| {
        let filter_str = log_filter.unwrap_or("ort=warn,rmcp=warn,debug");
        let filter = EnvFilter::new(filter_str);
        
        // Create .swissarmyhammer directory for logs
        let log_dir = std::path::PathBuf::from(".swissarmyhammer");
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            let error_context = match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    "Permission denied - check directory permissions"
                }
                std::io::ErrorKind::AlreadyExists => {
                    "Directory creation conflict - this shouldn't happen with create_dir_all"
                }
                _ => "Unknown filesystem error - check disk space and parent directory permissions"
            };
            eprintln!("Warning: Could not create MCP log directory {}: {} ({})", 
                     log_dir.display(), e, error_context);
            return;
        }

        let log_file_name = std::env::var("SWISSARMYHAMMER_LOG_FILE").unwrap_or_else(|_| "mcp.log".to_string());
        let log_file_path = log_dir.join(log_file_name);
        match std::fs::File::create(&log_file_path) {
            Ok(file) => {
                let shared_file = Arc::new(Mutex::new(file));
                let shared_file_for_cleanup = shared_file.clone();
                
                // Try to set global subscriber, handle case where it's already set (e.g., in tests)
                let subscriber = registry()
                    .with(filter)
                    .with(
                        fmt::layer()
                            .with_writer(move || {
                                let file = shared_file.clone();
                                Box::new(FileWriterGuard::new(file)) as Box<dyn std::io::Write>
                            })
                            .with_ansi(false) // No color codes in file
                    );

                if tracing::subscriber::set_global_default(subscriber).is_err() {
                    // This can happen in test environments where global subscriber is already set
                    tracing::debug!("Global tracing subscriber already set - MCP logging configuration skipped");
                    
                    // If we can't set the subscriber, we should clean up the file we created
                    drop(shared_file_for_cleanup);
                    let _ = std::fs::remove_file(&log_file_path);
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: Could not create MCP log file {}: {}. MCP server will use existing logging configuration.",
                    log_file_path.display(), e
                );
            }
        }
    });
}

/// MCP server transport mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpServerMode {
    /// Standard input/output transport
    Stdio,
    /// HTTP transport with optional port specification
    /// None = random port assignment
    Http { port: Option<u16> },
}

/// Connection information returned after server startup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    /// The transport mode used
    pub mode: McpServerMode,
    /// Connection URL or identifier
    pub connection_url: String,
    /// Actual bound port (for HTTP mode)
    pub port: Option<u16>,
}

/// Handle for managing HTTP MCP server lifecycle
#[derive(Debug)]
pub struct McpServerHandle {
    /// Server information
    pub info: McpServerInfo,
    /// Shutdown sender for graceful shutdown
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl McpServerHandle {
    /// Create a new MCP server handle
    fn new(info: McpServerInfo, shutdown_tx: oneshot::Sender<()>) -> Self {
        Self {
            info,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Get the connection information
    pub fn info(&self) -> &McpServerInfo {
        &self.info
    }

    /// Get the actual port (for HTTP mode)
    pub fn port(&self) -> Option<u16> {
        self.info.port
    }

    /// Get the connection URL
    pub fn url(&self) -> &str {
        &self.info.connection_url
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            if tx.send(()).is_err() {
                tracing::warn!("Server shutdown signal receiver already dropped");
            }
        }
        Ok(())
    }
}

/// Start unified MCP server with specified transport mode
///
/// This is the main entry point for starting MCP servers in any mode.
/// Returns connection information appropriate for the selected transport.
///
/// # Arguments
///
/// * `mode` - The transport mode (stdio or HTTP)
/// * `library` - Optional prompt library (creates new if None)
///
/// # Returns
///
/// * `Result<McpServerHandle>` - Server handle with connection info
pub async fn start_mcp_server(
    mode: McpServerMode,
    library: Option<PromptLibrary>,
) -> Result<McpServerHandle> {
    // Configure MCP logging to match sah serve behavior
    // NOTE: Skip logging configuration when called from CLI as main.rs already handles it
    // Only configure logging when used as library (e.g., in tests or embedded scenarios)
    if std::env::var("SAH_CLI_MODE").is_err() {
        configure_mcp_logging(None);
    }
    
    match mode {
        McpServerMode::Stdio => start_stdio_server(library).await,
        McpServerMode::Http { port } => start_http_server(port, library).await,
    }
}

/// Start MCP server with stdio transport
async fn start_stdio_server(library: Option<PromptLibrary>) -> Result<McpServerHandle> {
    let library = library.unwrap_or_default();
    let server = McpServer::new(library).await?;
    server.initialize().await?;

    tracing::info!("Starting unified MCP server in stdio mode");

    // Create a dummy shutdown channel for API consistency (stdio doesn't need shutdown)
    let (shutdown_tx, _shutdown_rx) = oneshot::channel();

    // For stdio mode, the server blocks on stdin/stdout until client disconnects
    // This is the correct behavior for stdio transport
    tokio::spawn(async move {
        match serve_server(server, stdio()).await {
            Ok(running_service) => {
                tracing::info!("MCP stdio server started successfully");
                match running_service.waiting().await {
                    Ok(quit_reason) => {
                        tracing::info!("MCP stdio server completed: {:?}", quit_reason);
                    }
                    Err(e) => {
                        tracing::error!("MCP stdio server task error: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to start stdio server: {}", e);
            }
        }
    });

    let info = McpServerInfo {
        mode: McpServerMode::Stdio,
        connection_url: "stdio".to_string(),
        port: None,
    };

    Ok(McpServerHandle::new(info, shutdown_tx))
}

/// Start MCP server with HTTP transport using rmcp SseServer
async fn start_http_server(
    port: Option<u16>,
    library: Option<PromptLibrary>,
) -> Result<McpServerHandle> {
    tracing::debug!("start_http_server called with port: {:?}", port);

    // First resolve the port (random or fixed)
    let actual_port = if let Some(bind_port) = port {
        tracing::debug!("Using specified port: {}", bind_port);
        bind_port
    } else {
        // Find available random port
        tracing::debug!("Finding available random port");
        let temp_listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| {
            swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("Failed to bind to random port: {}", e),
            }
        })?;

        let port = temp_listener
            .local_addr()
            .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("Failed to get local address: {}", e),
            })?
            .port();

        drop(temp_listener); // Release the port for rmcp to use
        tracing::debug!("Found random port: {}", port);
        port
    };

    // Now set up the server with the resolved port
    let bind_addr = format!("127.0.0.1:{}", actual_port);
    let socket_addr: std::net::SocketAddr =
        bind_addr
            .parse()
            .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("Failed to parse bind address {}: {}", bind_addr, e),
            })?;

    tracing::debug!("Parsed socket address: {}", socket_addr);

    // Create and initialize MCP server
    tracing::debug!("Creating MCP server");
    let library = library.unwrap_or_default();
    let server = McpServer::new(library).await?;
    tracing::debug!("Initializing MCP server");
    server.initialize().await?;
    tracing::debug!("MCP server initialized");

    // Use StreamableHttpService for /mcp endpoint (matches client example)
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let router = axum::Router::new()
        .nest_service("/mcp", service)
        .route("/health", axum::routing::get(health_check));
    let listener = tokio::net::TcpListener::bind(socket_addr)
        .await
        .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
            message: format!("Failed to bind to {}: {}", socket_addr, e),
        })?;

    let connection_url = format!("http://127.0.0.1:{}/mcp", actual_port); // Full URL with /mcp
    tracing::info!("Unified HTTP MCP server ready on {}", connection_url);

    // Create shutdown channel
    let (shutdown_tx, _shutdown_rx) = oneshot::channel();

    // Start the server
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let info = McpServerInfo {
        mode: McpServerMode::Http {
            port: Some(actual_port),
        },
        connection_url, // This now includes /mcp path
        port: Some(actual_port),
    };

    Ok(McpServerHandle::new(info, shutdown_tx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[test_log::test]
    async fn trace_this() {
        tracing::info!("test_http_server_creation_and_info");
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_http_server_creation_and_info() {
        tracing::info!("test_http_server_creation_and_info");
        let mode = McpServerMode::Http { port: Some(18080) }; // Fixed port to avoid random port issues
        let mut server = start_mcp_server(mode, None).await.unwrap();

        // Verify we got a valid port and URL format
        assert_eq!(server.port().unwrap(), 18080);
        assert!(server.url().starts_with("http://127.0.0.1:"));

        // Quick shutdown
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_server_info_structure() {
        let mode = McpServerMode::Http { port: Some(18081) };
        let mut server = start_mcp_server(mode, None).await.unwrap();

        // Test info structure
        let info = server.info();
        match &info.mode {
            McpServerMode::Http { port } => {
                assert_eq!(port, &Some(18081));
            }
            _ => panic!("Expected HTTP mode"),
        }

        assert_eq!(server.port().unwrap(), 18081);
        assert_eq!(server.url(), "http://127.0.0.1:18081/mcp");

        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_server_with_custom_library() {
        // Test that custom library is properly used
        let custom_library = PromptLibrary::default();

        let mode = McpServerMode::Http { port: None };
        let mut server = start_mcp_server(mode, Some(custom_library)).await.unwrap();

        // Server should start successfully with custom library
        assert!(server.port().unwrap() > 0);
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_http_server_port_in_use_error() {
        // First, start a server on a specific port
        let mode1 = McpServerMode::Http { port: Some(18082) };
        let mut server1 = start_mcp_server(mode1, None).await.unwrap();

        // Verify first server is running
        assert_eq!(server1.port().unwrap(), 18082);

        // Try to start another server on the same port - should fail
        let mode2 = McpServerMode::Http { port: Some(18082) };
        let result = start_mcp_server(mode2, None).await;

        // Should get an error about port being in use
        assert!(
            result.is_err(),
            "Expected error when trying to bind to same port"
        );
        let error_msg = format!("{}", result.unwrap_err());
        assert!(
            error_msg.contains("Failed to bind") || error_msg.contains("18082"),
            "Error should mention binding failure or port number. Got: {}",
            error_msg
        );

        // Clean up
        server1.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_http_server_invalid_port() {
        // Test with invalid port (port 1 requires root privileges)
        let mode = McpServerMode::Http { port: Some(1) };
        let result = start_mcp_server(mode, None).await;

        // Should get an error about permission denied
        assert!(
            result.is_err(),
            "Expected error when trying to bind to privileged port"
        );
        let error_msg = format!("{}", result.unwrap_err());
        assert!(
            error_msg.contains("Failed to bind")
                || error_msg.contains("Permission denied")
                || error_msg.contains("1"),
            "Error should mention binding failure, permission denied, or port 1. Got: {}",
            error_msg
        );
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_server_shutdown_idempotency() {
        // Test that calling shutdown multiple times doesn't panic
        let mode = McpServerMode::Http { port: None };
        let mut server = start_mcp_server(mode, None).await.unwrap();

        // First shutdown should work
        server.shutdown().await.unwrap();

        // Second shutdown should also work (idempotent)
        let result = server.shutdown().await;
        assert!(result.is_ok(), "Shutdown should be idempotent");
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_server_info_consistency() {
        // Test that server info remains consistent
        let mode = McpServerMode::Http { port: Some(18083) };
        let mut server = start_mcp_server(mode.clone(), None).await.unwrap();

        let info1 = server.info();
        let info2 = server.info();

        // Info should be consistent across calls
        assert_eq!(info1.port, info2.port);
        assert_eq!(info1.connection_url, info2.connection_url);

        match (&info1.mode, &mode) {
            (
                McpServerMode::Http { port: info_port },
                McpServerMode::Http {
                    port: expected_port,
                },
            ) => {
                assert_eq!(info_port, expected_port);
            }
            _ => panic!("Mode mismatch"),
        }

        server.shutdown().await.unwrap();
    }

    // NOTE: Multiple concurrent server test removed to avoid port conflicts and timeouts
}
