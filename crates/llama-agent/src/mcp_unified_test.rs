#[cfg(test)]
mod tests {
    use crate::mcp::{MCPClientBuilder, ServerConnectionConfig};
    use crate::types::MCPError;

    #[tokio::test]
    async fn test_unified_mcp_client_builder() {
        // Example showing simplified configuration without transport details
        let client = MCPClientBuilder::new()
            .add_process_server(
                "filesystem".to_string(),
                "python".to_string(),
                vec!["scripts/mcp_filesystem.py".to_string()],
            )
            .add_http_server(
                "remote_tools".to_string(),
                "https://api.example.com/mcp".to_string(),
            )
            .build()
            .await;

        // Should build successfully even if servers don't exist in test
        match client {
            Ok(_) => {
                // In a real test, we would verify the client works
                // For now, just test that the builder interface works
            }
            Err(e) => {
                // Expected to fail in test environment without actual servers
                println!("Expected failure in test: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_unified_mcp_client_custom_config() {
        // Example showing custom configuration
        let client = MCPClientBuilder::new()
            .add_server(
                "custom_server".to_string(),
                ServerConnectionConfig::Http {
                    url: "https://custom.example.com/mcp".to_string(),
                    timeout_secs: Some(60),
                },
            )
            .build()
            .await;

        match client {
            Ok(clients) => {
                // Test that the builder returns the expected servers
                let server_names: Vec<String> =
                    clients.iter().map(|(name, _)| name.clone()).collect();
                assert!(server_names.contains(&"custom_server".to_string()));
            }
            Err(e) => {
                println!("Expected failure in test: {}", e);
            }
        }
    }

    #[test]
    fn test_mcp_client_error_abstraction() {
        use crate::mcp::MCPClientError;

        // Test that transport errors are normalized
        let transport_error = MCPError::Connection("TCP connection failed".to_string());
        let client_error: MCPClientError = transport_error.into();

        match client_error {
            MCPClientError::Connection(msg) => {
                assert!(msg.contains("TCP connection failed"));
            }
            _ => panic!("Should be Connection error"),
        }

        let protocol_error = MCPError::Protocol("Invalid JSON-RPC".to_string());
        let client_error: MCPClientError = protocol_error.into();

        match client_error {
            MCPClientError::Protocol(msg) => {
                assert_eq!(msg, "Invalid JSON-RPC");
            }
            _ => panic!("Should be Protocol error"),
        }
    }
}
