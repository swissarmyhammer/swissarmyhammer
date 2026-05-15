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

pub mod daemon;
pub mod error;
pub mod registry;
pub mod supervisor;
pub mod types;
pub mod yaml_loader;

pub use daemon::LspDaemon;
pub use error::LspError;
pub use registry::{all_servers, servers_for_extensions, servers_for_project, SERVERS};
pub use supervisor::LspSupervisorManager;
pub use types::{DaemonStatus, LspDaemonState, LspServerSpec, OwnedLspServerSpec};
