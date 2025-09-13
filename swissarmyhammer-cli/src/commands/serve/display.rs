//! Display objects for serve command output
//!
//! Provides clean display objects with `Tabled` and `Serialize` derives for consistent
//! output formatting across table, JSON, and YAML formats.

use serde::{Deserialize, Serialize};
use tabled::Tabled;

/// Basic server status information for serve command output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct ServerStatus {
    #[tabled(rename = "Type")]
    pub server_type: String,

    #[tabled(rename = "Status")]
    pub status: String,

    #[tabled(rename = "Address")]
    pub address: String,

    #[tabled(rename = "Message")]
    pub message: String,
}

/// Detailed server status information for verbose serve command output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct VerboseServerStatus {
    #[tabled(rename = "Type")]
    pub server_type: String,

    #[tabled(rename = "Status")]
    pub status: String,

    #[tabled(rename = "Address")]
    pub address: String,

    #[tabled(rename = "Port")]
    pub port: String,

    #[tabled(rename = "Health URL")]
    pub health_url: String,

    #[tabled(rename = "Prompts")]
    pub prompt_count: usize,

    #[tabled(rename = "Message")]
    pub message: String,
}

impl ServerStatus {
    /// Create a new server status entry
    pub fn new(server_type: String, status: String, address: String, message: String) -> Self {
        Self {
            server_type,
            status,
            address,
            message,
        }
    }
}

impl VerboseServerStatus {
    /// Create a new verbose server status entry
    pub fn new(
        server_type: String,
        status: String,
        address: String,
        port: Option<u16>,
        health_url: Option<String>,
        prompt_count: usize,
        message: String,
    ) -> Self {
        Self {
            server_type,
            status,
            address,
            port: port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string()),
            health_url: health_url.unwrap_or_else(|| "-".to_string()),
            prompt_count,
            message,
        }
    }
}
