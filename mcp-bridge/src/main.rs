//! HTTP-to-stdio MCP Bridge
//! 
//! This bridge allows llama-agent to communicate with HTTP MCP servers
//! by converting stdio MCP protocol to HTTP requests.

use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use tokio::runtime::Runtime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get MCP server URL from command line argument
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: mcp-bridge <http_mcp_server_url>");
        std::process::exit(1);
    }
    
    let mcp_server_url = &args[1];
    eprintln!("MCP Bridge: Connecting to HTTP MCP server at {}", mcp_server_url);
    
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    
    // Read JSON-RPC requests from stdin and forward to HTTP MCP server
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        
        eprintln!("MCP Bridge: Received request: {}", line);
        
        // Parse JSON-RPC request
        let request: Value = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("MCP Bridge: Failed to parse JSON request: {}", e);
                continue;
            }
        };
        
        // Forward request to HTTP MCP server
        let client = reqwest::Client::new();
        let response = client
            .post(mcp_server_url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await;
            
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<Value>().await {
                        Ok(json_resp) => {
                            let response_str = serde_json::to_string(&json_resp)?;
                            eprintln!("MCP Bridge: Sending response: {}", response_str);
                            writeln!(stdout, "{}", response_str)?;
                            stdout.flush()?;
                        }
                        Err(e) => {
                            eprintln!("MCP Bridge: Failed to parse response JSON: {}", e);
                            let error_response = json!({
                                "jsonrpc": "2.0",
                                "id": request.get("id"),
                                "error": {
                                    "code": -32603,
                                    "message": format!("Failed to parse response: {}", e)
                                }
                            });
                            writeln!(stdout, "{}", serde_json::to_string(&error_response)?)?;
                            stdout.flush()?;
                        }
                    }
                } else {
                    eprintln!("MCP Bridge: HTTP error: {}", resp.status());
                    let error_response = json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "error": {
                            "code": -32603,
                            "message": format!("HTTP error: {}", resp.status())
                        }
                    });
                    writeln!(stdout, "{}", serde_json::to_string(&error_response)?)?;
                    stdout.flush()?;
                }
            }
            Err(e) => {
                eprintln!("MCP Bridge: HTTP request failed: {}", e);
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32603,
                        "message": format!("HTTP request failed: {}", e)
                    }
                });
                writeln!(stdout, "{}", serde_json::to_string(&error_response)?)?;
                stdout.flush()?;
            }
        }
    }
    
    Ok(())
}