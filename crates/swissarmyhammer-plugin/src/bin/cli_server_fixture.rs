//! A minimal stdio MCP server used as a test fixture for [`CliServer`].
//!
//! The [`CliServer`] transport spawns a child process and speaks MCP JSON-RPC
//! over its stdio. Exercising that transport honestly requires a *real* MCP
//! server on the other end of the pipe — this binary is exactly that.
//!
//! It is a genuine `rmcp` server built with the `#[tool_router]` / `#[tool]` /
//! `#[tool_handler]` macro stack, serving over [`rmcp::transport::io::stdio`].
//! It exposes a single flat `echo` tool that returns its `message` argument
//! verbatim. Nothing here is hand-rolled JSON-RPC: the fixture leans on the
//! same `rmcp` server SDK the rest of the platform uses.
//!
//! The binary exists only so the `cli_server` integration test has a real
//! subprocess to drive. It is not part of the platform's public surface.
//!
//! [`CliServer`]: swissarmyhammer_plugin::CliServer

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::transport::io::stdio;
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use serde::{Deserialize, Serialize};

/// Arguments for the fixture's `echo` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EchoArgs {
    /// The payload echoed straight back to the caller.
    message: String,
}

/// The fixture's `rmcp` server handler.
///
/// A flat handler with one tool; it holds the macro-generated tool router and
/// nothing else, because the fixture has no state to keep.
#[derive(Clone)]
struct FixtureServer {
    /// The macro-generated tool router for this handler.
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl FixtureServer {
    /// Builds a [`FixtureServer`] with its tool router wired up.
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Echoes the `message` argument straight back to the caller.
    #[tool(name = "echo", description = "Echoes its message argument back.")]
    async fn echo(&self, Parameters(args): Parameters<EchoArgs>) -> String {
        args.message
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FixtureServer {}

/// Serves the fixture MCP server over stdio until the client disconnects.
///
/// The process spawns an `rmcp` service over the standard input/output pair,
/// runs the service loop to completion, and exits. When [`CliServer`] drops
/// its connection, the transport closes and this loop terminates cleanly.
///
/// [`CliServer`]: swissarmyhammer_plugin::CliServer
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = FixtureServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
