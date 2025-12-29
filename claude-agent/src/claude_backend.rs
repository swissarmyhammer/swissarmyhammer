//! Backend trait for Claude process interaction
//!
//! This module provides an abstraction layer for communicating with Claude.

use crate::claude_process::ClaudeProcess;
use crate::error::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Trait for Claude process backends
///
/// This trait abstracts the I/O operations with Claude.
#[async_trait]
pub trait ClaudeBackend: Send {
    /// Write a line to Claude
    async fn write_line(&mut self, line: &str) -> Result<()>;

    /// Read a line from Claude
    ///
    /// Returns `None` when the stream is exhausted
    async fn read_line(&mut self) -> Result<Option<String>>;

    /// Shutdown the backend
    async fn shutdown(&mut self) -> Result<()>;
}

/// Backend that uses real Claude process
///
/// This wraps a ClaudeProcess (via Arc) and provides the ClaudeBackend interface.
pub struct RealClaudeBackend {
    process: Arc<tokio::sync::Mutex<ClaudeProcess>>,
}

impl RealClaudeBackend {
    /// Create a new real backend from a process (shared ownership)
    pub fn new(process: Arc<tokio::sync::Mutex<ClaudeProcess>>) -> Self {
        Self { process }
    }
}

#[async_trait]
impl ClaudeBackend for RealClaudeBackend {
    async fn write_line(&mut self, line: &str) -> Result<()> {
        self.process.lock().await.write_line(line).await
    }

    async fn read_line(&mut self) -> Result<Option<String>> {
        self.process.lock().await.read_line().await
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Can't consume Arc - just return Ok
        // Process will be cleaned up by ProcessManager
        Ok(())
    }
}
