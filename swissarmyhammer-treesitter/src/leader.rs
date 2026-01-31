//! Leader process that manages the tree-sitter workspace
//!
//! The leader:
//! - Owns the `IndexContext` with all parsed files and embeddings
//! - Listens on a Unix socket for client connections
//! - Handles queries via the tarpc `IndexService` trait
//! - Maintains file watchers to keep the workspace up to date

use std::path::Path;
use std::sync::Arc;

use futures::StreamExt;
use tarpc::server::{self, Channel};
use tokio::net::UnixListener;
use tokio::sync::{watch, RwLock};
use tokio_serde::formats::Bincode;

use crate::index::IndexContext;
use crate::query::server::IndexServiceServer;
use crate::query::service::IndexService;
use swissarmyhammer_leader_election::LeaderGuard;

/// The workspace leader server
///
/// Holds the index context and serves queries from clients.
/// Only one leader exists per workspace - other processes connect as clients.
pub struct WorkspaceLeader {
    /// The index context (owned by leader)
    index: Arc<RwLock<IndexContext>>,
    /// Shutdown signal sender
    shutdown_tx: watch::Sender<bool>,
    /// Leader guard (holds the lock)
    _guard: LeaderGuard,
}

impl WorkspaceLeader {
    /// Create a new leader with the given guard and workspace root
    ///
    /// This will scan and index the workspace on creation.
    pub async fn new(guard: LeaderGuard, workspace_root: impl AsRef<Path>) -> crate::Result<Self> {
        let mut index = IndexContext::new(workspace_root);
        index.scan().await?;

        let (shutdown_tx, _) = watch::channel(false);

        Ok(Self {
            index: Arc::new(RwLock::new(index)),
            shutdown_tx,
            _guard: guard,
        })
    }

    /// Run the leader server on the given socket path
    ///
    /// This will listen for client connections and handle queries until shutdown.
    pub async fn run(self, socket_path: &Path) -> crate::Result<()> {
        let _ = std::fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path).map_err(crate::TreeSitterError::Io)?;

        tracing::info!("Leader listening on {}", socket_path.display());

        let index = self.index.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, _)) => {
                            spawn_connection_handler(stream, index.clone());
                        }
                        Err(e) => {
                            tracing::warn!("Failed to accept connection: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Leader shutting down");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Signal the leader to shut down
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Get a reference to the index (for testing)
    #[cfg(test)]
    pub fn index(&self) -> &Arc<RwLock<IndexContext>> {
        &self.index
    }
}

/// Spawn a task to handle a client connection.
///
/// Sets up the tarpc transport over the Unix socket and runs the service.
fn spawn_connection_handler(stream: tokio::net::UnixStream, index: Arc<RwLock<IndexContext>>) {
    let codec_builder = Bincode::default;

    tokio::spawn(async move {
        let framed = tokio_util::codec::Framed::new(
            stream,
            tarpc::tokio_util::codec::LengthDelimitedCodec::new(),
        );
        let transport = tarpc::serde_transport::new(framed, codec_builder());

        let server = IndexServiceServer::new(index);
        let channel = server::BaseChannel::with_defaults(transport);

        channel
            .execute(server.serve())
            .for_each(|response| async move {
                tokio::spawn(response);
            })
            .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_leader_election::LeaderElection;
    use tempfile::TempDir;

    /// Time to wait for the socket server to start accepting connections
    const SOCKET_STARTUP_DELAY_MS: u64 = 100;

    // =========================================================================
    // WorkspaceLeader::new tests
    // =========================================================================

    #[tokio::test]
    async fn test_workspace_leader_new_empty_directory() {
        let dir = TempDir::new().unwrap();
        let election = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();

        let leader = WorkspaceLeader::new(guard, dir.path()).await.unwrap();

        // Verify the index is created
        let index = leader.index.read().await;
        assert!(index.files().is_empty());
    }

    #[tokio::test]
    async fn test_workspace_leader_new_with_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let election = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();

        let leader = WorkspaceLeader::new(guard, dir.path()).await.unwrap();

        // Verify files are indexed
        let index = leader.index.read().await;
        assert!(!index.files().is_empty());
    }

    #[tokio::test]
    async fn test_workspace_leader_new_scans_workspace() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("lib.rs"), "pub fn lib() {}").unwrap();

        let election = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();

        let leader = WorkspaceLeader::new(guard, dir.path()).await.unwrap();

        let index = leader.index.read().await;
        assert_eq!(index.files().len(), 2);
    }

    // =========================================================================
    // WorkspaceLeader::run tests
    // =========================================================================

    #[tokio::test]
    async fn test_workspace_leader_run_creates_socket() {
        let dir = TempDir::new().unwrap();
        let socket_path = dir.path().join("test.sock");

        let election = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        let leader = WorkspaceLeader::new(guard, dir.path()).await.unwrap();

        // Clone shutdown_tx before moving leader
        let shutdown_tx = leader.shutdown_tx.clone();

        // Run the server in a background task
        let socket_path_clone = socket_path.clone();
        let handle = tokio::spawn(async move {
            leader.run(&socket_path_clone).await
        });

        // Give the server time to start
        tokio::time::sleep(std::time::Duration::from_millis(SOCKET_STARTUP_DELAY_MS)).await;

        // Socket should exist
        assert!(socket_path.exists());

        // Signal shutdown
        let _ = shutdown_tx.send(true);

        // Wait for server to stop
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_workspace_leader_run_removes_stale_socket() {
        let dir = TempDir::new().unwrap();
        let socket_path = dir.path().join("test.sock");

        // Create a stale socket file
        std::fs::write(&socket_path, "stale").unwrap();

        let election = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        let leader = WorkspaceLeader::new(guard, dir.path()).await.unwrap();

        let shutdown_tx = leader.shutdown_tx.clone();

        let socket_path_clone = socket_path.clone();
        let handle = tokio::spawn(async move {
            leader.run(&socket_path_clone).await
        });

        tokio::time::sleep(std::time::Duration::from_millis(SOCKET_STARTUP_DELAY_MS)).await;

        // Should have replaced the stale file with a real socket
        assert!(socket_path.exists());

        let _ = shutdown_tx.send(true);
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_workspace_leader_run_accepts_connections() {
        use tokio::net::UnixStream;

        let dir = TempDir::new().unwrap();
        let socket_path = dir.path().join("test.sock");

        let election = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        let leader = WorkspaceLeader::new(guard, dir.path()).await.unwrap();

        let shutdown_tx = leader.shutdown_tx.clone();

        let socket_path_clone = socket_path.clone();
        let handle = tokio::spawn(async move {
            leader.run(&socket_path_clone).await
        });

        tokio::time::sleep(std::time::Duration::from_millis(SOCKET_STARTUP_DELAY_MS)).await;

        // Try to connect
        let connect_result = UnixStream::connect(&socket_path).await;
        assert!(connect_result.is_ok());

        let _ = shutdown_tx.send(true);
        let _ = handle.await;
    }

    // =========================================================================
    // WorkspaceLeader::shutdown tests
    // =========================================================================

    #[tokio::test]
    async fn test_workspace_leader_shutdown_stops_server() {
        let dir = TempDir::new().unwrap();
        let socket_path = dir.path().join("test.sock");

        let election = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        let leader = WorkspaceLeader::new(guard, dir.path()).await.unwrap();

        // Get a reference to call shutdown
        let shutdown_tx = leader.shutdown_tx.clone();

        let socket_path_clone = socket_path.clone();
        let handle = tokio::spawn(async move {
            leader.run(&socket_path_clone).await
        });

        tokio::time::sleep(std::time::Duration::from_millis(SOCKET_STARTUP_DELAY_MS)).await;

        // Shutdown using the cloned sender
        let _ = shutdown_tx.send(true);

        // Server should exit cleanly
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), handle).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_workspace_leader_shutdown_is_idempotent() {
        let dir = TempDir::new().unwrap();

        let election = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        let leader = WorkspaceLeader::new(guard, dir.path()).await.unwrap();

        // Multiple shutdown calls should not panic
        leader.shutdown();
        leader.shutdown();
        leader.shutdown();
    }

}
