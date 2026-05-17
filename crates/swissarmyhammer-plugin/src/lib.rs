//! The SwissArmyHammer plugin platform.
//!
//! This crate hosts plugins as MCP (Model Context Protocol) servers and
//! routes work to them. Its two responsibilities are:
//!
//! - **Registration** — plugins register and unregister MCP servers with the
//!   platform under unique names, making their tools and operations available.
//! - **Dispatch** — callers issue generic operation requests against a named
//!   server/tool/operation triple, and the platform dispatches them to the
//!   appropriate registered server.
//!
//! The modules below carry the pieces of that platform:
//!
//! - [`registry`] — tracks the set of registered MCP servers by name.
//! - [`dispatcher`] — routes generic operation requests to a registered server.
//! - [`server`] — the MCP server abstraction and its transports.
//! - [`runtime`] — the JavaScript runtime that hosts plugin code.
//! - [`host`] — host-side bindings exposed to plugins.
//! - [`ledger`] — records of registration and dispatch activity.
//! - [`codegen`] — code generation for plugin scaffolding and bindings.
//! - [`error`] — the platform [`Error`] type and [`Result`] alias.
//!
//! This is the scaffold crate; the module bodies are filled in by later work.

pub mod codegen;
pub mod dispatcher;
pub mod error;
pub mod host;
pub mod ledger;
pub mod registry;
pub mod runtime;
pub mod server;

pub use dispatcher::Dispatcher;
pub use error::{Error, Result};
pub use registry::{ServerName, ServerRegistry};
pub use runtime::{
    transpile_typescript, HostDispatcher, PluginRuntime, RuntimeConfig, TranspiledModule,
    UnboundHostDispatcher,
};
pub use server::{
    CallerId, CliServer, InProcessServer, McpServer, PluginId, ToolMetadata, UrlServer,
};
