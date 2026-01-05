//! Editor state management for accessing unsaved file buffers
//!
//! ACP requires integration with client editor state to access unsaved changes.
//! This module implements a protocol extension for querying and caching editor buffers.
//!
//! # ACP Editor Integration
//!
//! From the ACP specification:
//! > "These methods enable Agents to access unsaved editor state and allow Clients
//! > to track file modifications made during agent execution."
//!
//! This module provides:
//! 1. File reading includes unsaved editor changes when available
//! 2. Access to in-memory file buffers and modifications
//! 3. Real-time file content that reflects current editor state
//! 4. Integration with client workspace and editor management
//!
//! # Protocol Extension
//!
//! Since ACP 0.4.3 doesn't define a standard editor state protocol, this module
//! uses the `meta` extension points in `ClientCapabilities` to implement a custom
//! editor state query protocol.
//!
//! Clients can advertise editor state support via:
//! ```json
//! {
//!   "fs": {
//!     "readTextFile": true,
//!     "writeTextFile": true,
//!     "meta": {
//!       "editorState": true
//!     }
//!   }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

/// Unique identifier for an editor buffer query
pub type BufferQueryId = String;

/// Editor buffer with unsaved content
///
/// Represents the current state of a file in the client's editor, which may
/// differ from the file's content on disk due to unsaved modifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorBuffer {
    /// Absolute path to the file
    pub path: PathBuf,
    /// Current buffer content (may include unsaved changes)
    pub content: String,
    /// Whether buffer has unsaved modifications
    pub modified: bool,
    /// Last modification time
    pub last_modified: SystemTime,
    /// Character encoding (e.g., "UTF-8", "UTF-16")
    pub encoding: String,
}

/// Request to query editor buffers from client
///
/// Sent from agent to client to request the current state of editor buffers
/// for specific file paths. This allows the agent to access unsaved changes
/// when reading files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorBufferRequest {
    /// Session ID for validation
    pub session_id: String,
    /// Paths to query (must be absolute paths)
    pub paths: Vec<PathBuf>,
}

/// Response containing editor buffer state
///
/// Returned from client to agent with the current state of requested editor
/// buffers. Includes both available buffers and paths that don't have active
/// editor buffers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorBufferResponse {
    /// Available editor buffers indexed by path
    pub buffers: HashMap<PathBuf, EditorBuffer>,
    /// Paths that don't have editor buffers open
    pub unavailable_paths: Vec<PathBuf>,
}

/// Manager for editor state queries and caching
///
/// Manages communication with the client to query editor buffer state and
/// caches results to minimize repeated queries. Provides fallback to disk
/// reads when editor buffers are not available.
///
/// # Caching Strategy
///
/// Editor buffers are cached for a short duration (1 second by default) to
/// balance performance with data freshness. The cache is automatically
/// invalidated after the timeout period.
///
/// # Example
///
/// ```ignore
/// let manager = EditorStateManager::new();
/// let path = Path::new("/absolute/path/to/file.rs");
///
/// // Try to get content from editor buffer
/// match manager.get_file_content("123", path).await {
///     Ok(Some(buffer)) => {
///         // Use editor buffer content (includes unsaved changes)
///         process_content(&buffer.content);
///     }
///     Ok(None) => {
///         // No editor buffer, fall back to disk read
///         let content = tokio::fs::read_to_string(path).await?;
///         process_content(&content);
///     }
///     Err(e) => {
///         // Query failed, fall back to disk read
///         let content = tokio::fs::read_to_string(path).await?;
///         process_content(&content);
///     }
/// }
/// ```
pub struct EditorStateManager {
    /// Cache of editor buffers by path
    buffer_cache: Arc<RwLock<HashMap<PathBuf, CachedBuffer>>>,
    /// Cache expiration duration
    cache_duration: std::time::Duration,
}

/// Cached editor buffer with expiration tracking
#[derive(Debug, Clone)]
struct CachedBuffer {
    /// The cached editor buffer
    buffer: EditorBuffer,
    /// Time when this buffer was cached
    cached_at: SystemTime,
}

impl EditorStateManager {
    /// Create a new editor state manager with default cache duration (1 second)
    pub fn new() -> Self {
        Self {
            buffer_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_duration: std::time::Duration::from_secs(1),
        }
    }

    /// Create a new editor state manager with custom cache duration
    pub fn with_cache_duration(cache_duration: std::time::Duration) -> Self {
        Self {
            buffer_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_duration,
        }
    }

    /// Get file content, checking editor state first
    ///
    /// Returns:
    /// - `Ok(Some(buffer))` if an editor buffer is available (from cache)
    /// - `Ok(None)` if no editor buffer is available (file not open in editor)
    ///
    /// # ACP Compliance
    ///
    /// This method implements the ACP requirement to access unsaved editor state
    /// before falling back to disk content. It ensures agents work with current,
    /// not stale, file content.
    ///
    /// # Client Integration
    ///
    /// Clients should proactively push editor state updates via the
    /// `editor/update_buffers` extension method. The agent maintains a cache
    /// of editor buffers that have been pushed by the client.
    pub async fn get_file_content(
        &self,
        _session_id: &str,
        path: &Path,
    ) -> crate::Result<Option<EditorBuffer>> {
        // Check cache first
        if let Some(cached) = self.get_cached_buffer(path).await {
            tracing::trace!("Editor buffer cache hit for: {}", path.display());
            return Ok(Some(cached));
        }

        tracing::trace!("Editor buffer cache miss for: {}", path.display());

        // Return None to indicate editor buffer not available in cache
        // Clients can proactively push updates via editor/update_buffers
        Ok(None)
    }

    /// Update cached buffers from client response
    ///
    /// This method processes an `EditorBufferResponse` from the client and
    /// updates the internal cache with the provided buffers. This is typically
    /// called when handling the `editor/update_buffers` extension method.
    ///
    /// # Arguments
    ///
    /// * `response` - The editor buffer response containing buffers to cache
    ///
    /// # Behavior
    ///
    /// - All buffers in the response are cached with the current timestamp
    /// - Existing cached buffers for the same paths are replaced
    /// - Unavailable paths are not cached (they remain as cache misses)
    pub async fn update_buffers_from_response(&self, response: EditorBufferResponse) {
        let now = SystemTime::now();
        let mut cache = self.buffer_cache.write().await;

        for (path, buffer) in response.buffers {
            tracing::debug!("Caching editor buffer for: {}", path.display());
            cache.insert(
                path,
                CachedBuffer {
                    buffer,
                    cached_at: now,
                },
            );
        }

        for path in response.unavailable_paths {
            tracing::debug!("Removing unavailable buffer from cache: {}", path.display());
            cache.remove(&path);
        }
    }

    /// Get cached buffer if still valid
    async fn get_cached_buffer(&self, path: &Path) -> Option<EditorBuffer> {
        let cache = self.buffer_cache.read().await;

        if let Some(cached) = cache.get(path) {
            let now = SystemTime::now();
            if let Ok(elapsed) = now.duration_since(cached.cached_at) {
                if elapsed < self.cache_duration {
                    return Some(cached.buffer.clone());
                }
            }
        }

        None
    }

    /// Cache an editor buffer
    ///
    /// Stores the buffer in the cache with the current timestamp. The buffer
    /// will be automatically invalidated after the cache duration expires.
    ///
    /// This method will be used by client protocol handlers when they receive
    /// editor buffer state from the client. Currently unused as client protocol
    /// communication is not yet implemented.
    pub async fn cache_buffer(&self, path: PathBuf, buffer: EditorBuffer) {
        let mut cache = self.buffer_cache.write().await;
        cache.insert(
            path,
            CachedBuffer {
                buffer,
                cached_at: SystemTime::now(),
            },
        );
    }

    /// Clear cache for a specific path
    ///
    /// Removes the cached editor buffer for the given path. This should be
    /// called when a file is written to ensure the cache doesn't contain
    /// stale data.
    pub async fn invalidate_cache(&self, path: &Path) {
        let mut cache = self.buffer_cache.write().await;
        cache.remove(path);
    }

    /// Clear all cached buffers
    ///
    /// Removes all cached editor buffers. Useful for testing or when the
    /// session state changes significantly.
    pub async fn clear_cache(&self) {
        let mut cache = self.buffer_cache.write().await;
        cache.clear();
    }

    /// Get the number of cached buffers
    ///
    /// Returns the current count of cached editor buffers, useful for
    /// monitoring and testing.
    pub async fn cache_size(&self) -> usize {
        let cache = self.buffer_cache.read().await;
        cache.len()
    }
}

impl Default for EditorStateManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if client capabilities include editor state support
///
/// Returns true if the client has advertised editor state support via the
/// `meta` extension point in `FileSystemCapability`.
///
/// # Example Client Capabilities
///
/// ```json
/// {
///   "fs": {
///     "readTextFile": true,
///     "writeTextFile": true,
///     "meta": {
///       "editorState": true
///     }
///   }
/// }
/// ```
pub fn supports_editor_state(capabilities: &agent_client_protocol::ClientCapabilities) -> bool {
    if let Some(meta) = &capabilities.fs.meta {
        if let Some(editor_state) = meta.get("editorState") {
            return editor_state.as_bool().unwrap_or(false);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_editor_state_manager_new() {
        let manager = EditorStateManager::new();
        assert_eq!(manager.cache_size().await, 0);
    }

    #[tokio::test]
    async fn test_cache_buffer() {
        let manager = EditorStateManager::new();
        let path = PathBuf::from("/test/file.rs");
        let buffer = EditorBuffer {
            path: path.clone(),
            content: "test content".to_string(),
            modified: true,
            last_modified: SystemTime::now(),
            encoding: "UTF-8".to_string(),
        };

        manager.cache_buffer(path.clone(), buffer.clone()).await;
        assert_eq!(manager.cache_size().await, 1);

        let cached = manager.get_cached_buffer(&path).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().content, "test content");
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let manager = EditorStateManager::with_cache_duration(Duration::from_millis(50));
        let path = PathBuf::from("/test/file.rs");
        let buffer = EditorBuffer {
            path: path.clone(),
            content: "test content".to_string(),
            modified: true,
            last_modified: SystemTime::now(),
            encoding: "UTF-8".to_string(),
        };

        manager.cache_buffer(path.clone(), buffer).await;
        assert!(manager.get_cached_buffer(&path).await.is_some());

        // Wait for cache to expire
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(manager.get_cached_buffer(&path).await.is_none());
    }

    #[tokio::test]
    async fn test_invalidate_cache() {
        let manager = EditorStateManager::new();
        let path = PathBuf::from("/test/file.rs");
        let buffer = EditorBuffer {
            path: path.clone(),
            content: "test content".to_string(),
            modified: true,
            last_modified: SystemTime::now(),
            encoding: "UTF-8".to_string(),
        };

        manager.cache_buffer(path.clone(), buffer).await;
        assert_eq!(manager.cache_size().await, 1);

        manager.invalidate_cache(&path).await;
        assert_eq!(manager.cache_size().await, 0);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let manager = EditorStateManager::new();

        for i in 0..5 {
            let path = PathBuf::from(format!("/test/file{}.rs", i));
            let buffer = EditorBuffer {
                path: path.clone(),
                content: format!("content {}", i),
                modified: true,
                last_modified: SystemTime::now(),
                encoding: "UTF-8".to_string(),
            };
            manager.cache_buffer(path, buffer).await;
        }

        assert_eq!(manager.cache_size().await, 5);
        manager.clear_cache().await;
        assert_eq!(manager.cache_size().await, 0);
    }

    #[tokio::test]
    async fn test_get_file_content_no_cache() {
        let manager = EditorStateManager::new();
        let path = Path::new("/test/file.rs");

        let result = manager.get_file_content("123", path).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_supports_editor_state_true() {
        use agent_client_protocol::ClientCapabilities;
        use serde_json::json;

        let capabilities = ClientCapabilities::new()
            .fs(agent_client_protocol::FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(true)
                .meta(
                    json!({
                        "editorState": true
                    })
                    .as_object()
                    .cloned(),
                ))
            .terminal(false);

        assert!(supports_editor_state(&capabilities));
    }

    #[test]
    fn test_supports_editor_state_false() {
        use agent_client_protocol::ClientCapabilities;
        use serde_json::json;

        let capabilities = ClientCapabilities::new()
            .fs(agent_client_protocol::FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(true)
                .meta(
                    json!({
                        "editorState": false
                    })
                    .as_object()
                    .cloned(),
                ))
            .terminal(false);

        assert!(!supports_editor_state(&capabilities));
    }

    #[test]
    fn test_supports_editor_state_missing() {
        use agent_client_protocol::ClientCapabilities;

        let capabilities = ClientCapabilities::new()
            .fs(agent_client_protocol::FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(true))
            .terminal(false);

        assert!(!supports_editor_state(&capabilities));
    }

    #[tokio::test]
    async fn test_update_buffers_from_client() {
        let manager = EditorStateManager::new();
        let path1 = PathBuf::from("/test/file1.rs");
        let path2 = PathBuf::from("/test/file2.rs");

        let buffer1 = EditorBuffer {
            path: path1.clone(),
            content: "content 1".to_string(),
            modified: true,
            last_modified: SystemTime::now(),
            encoding: "UTF-8".to_string(),
        };

        let buffer2 = EditorBuffer {
            path: path2.clone(),
            content: "content 2".to_string(),
            modified: false,
            last_modified: SystemTime::now(),
            encoding: "UTF-8".to_string(),
        };

        let mut buffers = HashMap::new();
        buffers.insert(path1.clone(), buffer1);
        buffers.insert(path2.clone(), buffer2);

        let response = EditorBufferResponse {
            buffers,
            unavailable_paths: vec![],
        };

        manager.update_buffers_from_response(response).await;

        assert_eq!(manager.cache_size().await, 2);
        assert!(manager.get_cached_buffer(&path1).await.is_some());
        assert!(manager.get_cached_buffer(&path2).await.is_some());
    }

    #[tokio::test]
    async fn test_get_file_content_with_client_pushed_cache() {
        let manager = EditorStateManager::new();
        let path = PathBuf::from("/test/file.rs");

        let buffer = EditorBuffer {
            path: path.clone(),
            content: "cached content".to_string(),
            modified: true,
            last_modified: SystemTime::now(),
            encoding: "UTF-8".to_string(),
        };

        manager.cache_buffer(path.clone(), buffer).await;

        let result = manager.get_file_content("123", &path).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().content, "cached content");
    }
}
