//! Test client for ACP conformance testing
//!
//! This module provides utilities for testing ACP agents via stdio streams.
//! Agents can be tested in-process or as separate processes.

use agent_client_protocol::Client;
use tokio::io::{AsyncRead, AsyncWrite};

/// A test client that communicates with an agent via streams
///
/// This wraps the ACP Client and provides a convenient interface for conformance testing.
pub struct TestClient {
    /// The ACP client
    client: Client,
}

impl TestClient {
    /// Create a client from input and output streams
    ///
    /// # Arguments
    /// * `stdin` - Stream to write requests to the agent
    /// * `stdout` - Stream to read responses from the agent
    pub fn new<R, W>(stdin: W, stdout: R) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let client = Client::new(stdin, stdout);
        Self { client }
    }

    /// Get a reference to the ACP client
    pub fn client(&self) -> &Client {
        &self.client
    }
}
