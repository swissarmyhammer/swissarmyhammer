//! LSP server lifecycle management for SwissArmyHammer.
//!
//! Provides zero-config LSP server detection, startup, health-checking,
//! and restart. Servers are auto-detected based on project type from
//! `swissarmyhammer-project-detection`.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use std::path::PathBuf;
//! use swissarmyhammer_lsp::LspSupervisorManager;
//!
//! # async fn run() {
//! let mut mgr = LspSupervisorManager::new(PathBuf::from("/my/workspace"));
//! let results = mgr.start().await;
//! for r in &results {
//!     if let Err(e) = r {
//!         eprintln!("LSP start error: {e}");
//!     }
//! }
//! // ... later ...
//! mgr.shutdown().await;
//! # }
//! ```

pub mod client;
pub mod daemon;
pub mod diagnostics;
pub mod error;
pub mod registry;
pub mod server_spec;
pub mod session;
pub mod severity;
pub mod supervisor;
pub mod types;
pub mod uri;
pub mod yaml_loader;

#[cfg(test)]
pub(crate) mod test_support;

pub use client::{parse_document_symbols, LspJsonRpcClient, LspTransport, SharedLspClient};
pub use daemon::LspDaemon;
pub use diagnostics::{parse_diagnostics_from_result, parse_publish_diagnostics, DiagnosticUpdate};
pub use error::LspError;
pub use registry::{all_servers, servers_for_extensions, servers_for_project, SERVERS};
pub use server_spec::{
    builtin_lsp_yaml_sources, detect_rust_analyzer, find_executable, load_lsp_servers,
    start_lsp_server, LspServerConfig, LspServerHandle, LSP_REGISTRY,
};
pub use session::LspSession;
pub use severity::DiagnosticSeverity;
pub use supervisor::LspSupervisorManager;
pub use types::{DaemonStatus, LspDaemonState, LspServerSpec, OwnedLspServerSpec};
pub use uri::{file_path_from_uri, file_uri_from_path};
