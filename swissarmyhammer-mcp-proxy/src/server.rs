use crate::proxy::FilteringMcpProxy;
use axum::Router;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::StreamableHttpService;
use std::sync::Arc;
use tokio::net::TcpListener;

/// Start an HTTP server for the FilteringMcpProxy.
///
/// # Arguments
///
/// * `proxy` - The filtering proxy to serve
/// * `port` - Optional port to bind to. If None, a random available port will be used.
///
/// # Returns
///
/// Returns a tuple of (actual_port, shutdown_handle) where:
/// - actual_port: The port the server is listening on
/// - shutdown_handle: JoinHandle for the server task (abort to shutdown)
///
/// # Example
///
/// ```no_run
/// use swissarmyhammer_mcp_proxy::{FilteringMcpProxy, ToolFilter, start_proxy_server};
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let server = Arc::new(todo!("create McpServer"));
/// let filter = ToolFilter::new(vec!["^files_.*".to_string()], vec![])?;
/// let proxy = FilteringMcpProxy::new(server, filter);
///
/// let (port, handle) = start_proxy_server(Arc::new(proxy), None).await?;
/// println!("Proxy server listening on port {}", port);
///
/// // Use the proxy...
///
/// // Shutdown when done
/// handle.abort();
/// # Ok(())
/// # }
/// ```
pub async fn start_proxy_server(
    proxy: Arc<FilteringMcpProxy>,
    port: Option<u16>,
) -> Result<(u16, tokio::task::JoinHandle<()>), Box<dyn std::error::Error>> {
    // Resolve the port (random or fixed)
    let actual_port = if let Some(bind_port) = port {
        tracing::debug!("FilteringMcpProxy: Using specified port: {}", bind_port);
        bind_port
    } else {
        // Find available random port
        tracing::debug!("FilteringMcpProxy: Finding available random port");
        let temp_listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = temp_listener.local_addr()?.port();
        drop(temp_listener); // Release the port for binding
        tracing::debug!("FilteringMcpProxy: Found random port: {}", port);
        port
    };

    let bind_addr = format!("127.0.0.1:{}", actual_port);
    let socket_addr: std::net::SocketAddr = bind_addr.parse()?;

    tracing::debug!(
        "FilteringMcpProxy: Binding to socket address: {}",
        socket_addr
    );

    // Create StreamableHttpService for the proxy
    let proxy_for_service = proxy.clone();
    let service = StreamableHttpService::new(
        move || Ok((*proxy_for_service).clone()),
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );

    // Create router with /mcp and /health endpoints
    let router = Router::new()
        .nest_service("/mcp", service)
        .route("/health", axum::routing::get(health_check));

    let listener = TcpListener::bind(socket_addr).await?;

    let connection_url = format!("http://127.0.0.1:{}/mcp", actual_port);
    tracing::info!(
        "FilteringMcpProxy HTTP server listening on {}",
        connection_url
    );

    // Start the server task
    let server_task = tokio::spawn(async move {
        tracing::info!("FilteringMcpProxy HTTP server task started");

        let result = axum::serve(listener, router).await;

        match result {
            Ok(_) => {
                tracing::info!("FilteringMcpProxy HTTP server completed successfully");
            }
            Err(e) => {
                tracing::error!("FilteringMcpProxy HTTP server error: {}", e);
            }
        }

        tracing::info!("FilteringMcpProxy HTTP server task exiting");
    });

    Ok((actual_port, server_task))
}

/// Health check handler for the /health endpoint.
async fn health_check() -> &'static str {
    "OK"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FilteringMcpProxy, ToolFilter};
    use swissarmyhammer_prompts::PromptLibrary;
    use swissarmyhammer_tools::mcp::McpServer;

    #[tokio::test]
    async fn test_start_proxy_server_with_random_port() {
        // Create a test server
        let library = PromptLibrary::default();
        let work_dir = std::env::current_dir().unwrap();
        let server = McpServer::new_with_work_dir(library, work_dir, None)
            .await
            .unwrap();
        server.initialize().await.unwrap();

        // Create filter allowing only files_read
        let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();

        // Create proxy
        let proxy = Arc::new(FilteringMcpProxy::new(Arc::new(server), filter));

        // Start server with random port
        let (port, handle) = start_proxy_server(proxy, None).await.unwrap();

        // Verify port is valid
        assert!(port > 0);

        // Cleanup
        handle.abort();
    }

    #[tokio::test]
    async fn test_health_check_endpoint() {
        let response = health_check().await;
        assert_eq!(response, "OK");
    }
}
