//! Display objects for serve command output
//!
//! Provides structured display objects for server status updates and information

use serde::{Deserialize, Serialize};
use tabled::Tabled;

/// Server status for basic output
#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct ServerStatus {
    /// Server type (HTTP or Stdio)
    #[tabled(rename = "Type")]
    pub server_type: String,
    
    /// Server status (Starting, Running, Stopping, Stopped, Error)
    #[tabled(rename = "Status")]
    pub status: String,
    
    /// Server address or connection info
    #[tabled(rename = "Address")]
    pub address: String,
    
    /// Additional message or details
    #[tabled(rename = "Message")]
    pub message: String,
}

/// Verbose server status with additional details
#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct VerboseServerStatus {
    /// Server type (HTTP or Stdio)
    #[tabled(rename = "Type")]
    pub server_type: String,
    
    /// Server status (Starting, Running, Stopping, Stopped, Error)
    #[tabled(rename = "Status")]
    pub status: String,
    
    /// Server address or connection info
    #[tabled(rename = "Address")]
    pub address: String,
    
    /// Port number (for HTTP servers)
    #[tabled(rename = "Port")]
    pub port: String,
    
    /// Health check URL (for HTTP servers)
    #[tabled(rename = "Health Check")]
    pub health_url: String,
    
    /// Number of loaded prompts
    #[tabled(rename = "Prompts")]
    pub prompt_count: usize,
    
    /// Additional message or details
    #[tabled(rename = "Message")]
    pub message: String,
}

impl ServerStatus {
    /// Create a new server status
    pub fn new(server_type: String, status: String, address: String, message: String) -> Self {
        Self {
            server_type,
            status,
            address,
            message,
        }
    }
    
    /// Create a starting status
    pub fn starting(server_type: String, address: String) -> Self {
        Self::new(
            server_type,
            "Starting".to_string(),
            address,
            "Server is starting up".to_string(),
        )
    }
    
    /// Create a running status
    pub fn running(server_type: String, address: String) -> Self {
        Self::new(
            server_type,
            "Running".to_string(),
            address,
            "Server is running successfully".to_string(),
        )
    }
    
    /// Create a stopping status
    pub fn stopping(server_type: String, address: String) -> Self {
        Self::new(
            server_type,
            "Stopping".to_string(),
            address,
            "Server is shutting down".to_string(),
        )
    }
    
    /// Create a stopped status
    pub fn stopped(server_type: String) -> Self {
        Self::new(
            server_type,
            "Stopped".to_string(),
            "-".to_string(),
            "Server stopped successfully".to_string(),
        )
    }
    
    /// Create an error status
    pub fn error(server_type: String, address: String, error_message: String) -> Self {
        Self::new(
            server_type,
            "Error".to_string(),
            address,
            format!("Error: {}", error_message),
        )
    }
}

impl VerboseServerStatus {
    /// Create a new verbose server status
    pub fn new(
        server_type: String,
        status: String,
        address: String,
        port: Option<u16>,
        health_url: Option<String>,
        prompt_count: usize,
        message: String,
    ) -> Self {
        let port_str = port.map_or("-".to_string(), |p| p.to_string());
        let health_url_str = health_url.unwrap_or("-".to_string());
        
        Self {
            server_type,
            status,
            address,
            port: port_str,
            health_url: health_url_str,
            prompt_count,
            message,
        }
    }
    
    /// Create a starting status with verbose info
    pub fn starting(server_type: String, address: String, port: Option<u16>, prompt_count: usize) -> Self {
        let health_url = if let Some(p) = port {
            Some(format!("http://{}:{}/health", address.split(':').next().unwrap_or("127.0.0.1"), p))
        } else {
            None
        };
        
        Self::new(
            server_type,
            "Starting".to_string(),
            address,
            port,
            health_url,
            prompt_count,
            "Server is starting up with loaded prompts".to_string(),
        )
    }
    
    /// Create a running status with verbose info
    pub fn running(server_type: String, address: String, port: Option<u16>, prompt_count: usize) -> Self {
        let health_url = if let Some(p) = port {
            Some(format!("http://{}:{}/health", address.split(':').next().unwrap_or("127.0.0.1"), p))
        } else {
            None
        };
        
        Self::new(
            server_type,
            "Running".to_string(),
            address,
            port,
            health_url,
            prompt_count,
            "Server is running with all tools available".to_string(),
        )
    }
    
    /// Create a stopping status with verbose info
    pub fn stopping(server_type: String, address: String) -> Self {
        Self::new(
            server_type,
            "Stopping".to_string(),
            address,
            None,
            None,
            0,
            "Server is gracefully shutting down".to_string(),
        )
    }
    
    /// Create a stopped status with verbose info
    pub fn stopped(server_type: String) -> Self {
        Self::new(
            server_type,
            "Stopped".to_string(),
            "-".to_string(),
            None,
            None,
            0,
            "Server stopped successfully and cleaned up resources".to_string(),
        )
    }
    
    /// Create an error status with verbose info
    pub fn error(server_type: String, address: String, error_message: String) -> Self {
        Self::new(
            server_type,
            "Error".to_string(),
            address,
            None,
            None,
            0,
            format!("Error: {}", error_message),
        )
    }
}