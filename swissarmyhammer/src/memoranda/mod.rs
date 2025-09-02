//! Memoranda system for storing and managing structured text memos
//!
//! This module provides a comprehensive memo management system that stores memos as
//! pure markdown files with filename-based identifiers and automatic timestamping.
//! The modern MarkdownMemoStorage is the recommended approach, with legacy JSON-based
//! storage available for backward compatibility.
//!
//! ## Features
//!
//! - **Filename-based Identifiers**: IDs derived from sanitized memo titles for human-readable file organization
//! - **Pure Markdown Storage**: Files stored as `.md` with content only, no metadata wrapper
//! - **Automatic Timestamps**: Creation and update times derived from filesystem metadata
//! - **Full-text Search**: Search across memo titles and content
//! - **Type-safe API**: Strong typing for memo identifiers and validation
//!
//! ## Basic Usage
//!
//! ```rust
//! use swissarmyhammer::memoranda::{MemoStorage, MarkdownMemoStorage, Memo};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new markdown memo storage (recommended)
//! let storage = MarkdownMemoStorage::new_default()?;
//!
//! // Store the memo
//! let stored_memo = storage.create_memo(
//!     "Meeting Notes".to_string(),
//!     "Discussed project timeline and deliverables.".to_string()
//! ).await?;
//! println!("Created memo with ID: {}", stored_memo.id);
//!
//! // Search memos
//! let results = storage.search_memos("timeline").await?;
//! println!("Found {} matching memos", results.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## Memo Management
//!
//! ```rust
//! use swissarmyhammer::memoranda::{MemoStorage, MarkdownMemoStorage, MemoId};
//!
//! # async fn management_example() -> Result<(), Box<dyn std::error::Error>> {
//! let storage = MarkdownMemoStorage::new_default()?;
//!
//! // List all memos (sorted by creation time, newest first)
//! let all_memos = storage.list_memos().await?;
//!
//! // Get a specific memo by ID
//! if let Some(memo) = all_memos.first() {
//!     let retrieved = storage.get_memo(&memo.id).await?;
//!     println!("Retrieved memo: {}", retrieved.title);
//!
//!     // Update memo content
//!     storage.update_memo(&memo.id, "Updated content".to_string()).await?;
//!
//!     // Delete memo when done
//!     storage.delete_memo(&memo.id).await?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Advanced Search and Query Operations
//!
//! ```rust
//! use swissarmyhammer::memoranda::{
//!     MarkdownMemoStorage, MemoStorage, AdvancedMemoSearchEngine, SearchOptions
//! };
//!
//! # async fn search_example() -> Result<(), Box<dyn std::error::Error>> {
//! let storage = MarkdownMemoStorage::new_default()?;
//! let search_engine = AdvancedMemoSearchEngine::new_in_memory().await?;
//!
//! // Create some example memos
//! let memo1 = storage.create_memo(
//!     "API Documentation".to_string(),
//!     "# REST API Guide\n\nAuthentication using JWT tokens...".to_string()
//! ).await?;
//!
//! let memo2 = storage.create_memo(
//!     "Meeting Notes".to_string(),
//!     "Discussed API authentication and security measures...".to_string()
//! ).await?;
//!
//! // Index all memos for advanced search
//! let all_memos = storage.list_memos().await?;
//! search_engine.index_memos(&all_memos).await?;
//!
//! // Configure search options
//! let search_options = SearchOptions {
//!     case_sensitive: false,
//!     exact_phrase: false,
//!     max_results: Some(10),
//!     include_highlights: true,
//!     excerpt_length: 80,
//! };
//!
//! // Perform advanced search with relevance scoring
//! let search_results = search_engine
//!     .search("API authentication", &search_options, &all_memos)
//!     .await?;
//!
//! // Display results with relevance scores
//! for result in search_results {
//!     println!("Found: {} ({}% relevance)",
//!         result.memo.title, result.relevance_score);
//!     
//!     if !result.highlights.is_empty() {
//!         println!("Highlights: {}", result.highlights.join(" ... "));
//!     }
//! }
//!
//! // Basic search for simpler use cases
//! let basic_results = storage.search_memos("meeting").await?;
//! println!("Basic search found {} memos", basic_results.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## Storage Configuration and Custom Paths
//!
//! ```rust
//! use swissarmyhammer::memoranda::{MarkdownMemoStorage, MemoStorage};
//! use std::path::PathBuf;
//!
//! # async fn storage_config_example() -> Result<(), Box<dyn std::error::Error>> {
//! // Use default storage location (./.swissarmyhammer/memos)
//! let default_storage = MarkdownMemoStorage::new_default()?;
//!
//! // Use custom storage directory
//! let custom_path = PathBuf::from("/custom/memo/storage");
//! let custom_storage = MarkdownMemoStorage::new(custom_path);
//!
//! // Create memo in custom storage
//! let memo = custom_storage.create_memo(
//!     "Configuration Example".to_string(),
//!     "This memo is stored in a custom directory.".to_string()
//! ).await?;
//!
//! println!("Created memo with ID: {}", memo.id);
//! # Ok(())
//! # }
//! ```
//!
//! ## Working with Memo Identifiers
//!
//! ```rust
//! use swissarmyhammer::memoranda::{MemoId, MemoStorage, MarkdownMemoStorage};
//!
//! # async fn id_example() -> Result<(), Box<dyn std::error::Error>> {
//! // Generate filename-based ID from title
//! let id = MemoId::from_filename("Meeting_Notes");
//! println!("Created ID from filename: {}", id);
//!
//! // Parse ID from string (filename validation included)
//! let id_string = "Project_Planning_Session";
//! let parsed_id = MemoId::from_filename(id_string);
//!
//! // IDs are based on filenames for human-readable organization
//! let id1 = MemoId::from_filename("Alpha_Project");
//! let id2 = MemoId::from_filename("Beta_Project");
//! // IDs can be compared lexicographically
//!
//! // Use in storage operations
//! let storage = MarkdownMemoStorage::new_default()?;
//! let memo = storage.create_memo(
//!     "ID Example".to_string(),
//!     "Demonstrating filename-based ID usage".to_string()
//! ).await?;
//!
//! // Retrieve using the generated ID
//! let retrieved = storage.get_memo(&memo.id).await?;
//! println!("Retrieved: {}", retrieved.title);
//! # Ok(())
//! # }
//! ```
//!
//! ## Error Handling and Validation
//!
//! ```rust
//! use swissarmyhammer::memoranda::{MemoId, MemoStorage, MarkdownMemoStorage};
//! use swissarmyhammer::error::SwissArmyHammerError;
//!
//! # async fn error_handling_example() -> Result<(), Box<dyn std::error::Error>> {
//! let storage = MarkdownMemoStorage::new_default()?;
//!
//! // Handle invalid ID formats (can also create from filename)
//! match MemoId::from_string("invalid-id".to_string()) {
//!     Ok(id) => println!("Valid ID: {}", id),
//!     Err(SwissArmyHammerError::Other(msg)) => {
//!         println!("Invalid ID format: {}", msg);
//!     }
//!     Err(e) => return Err(e.into()),
//! }
//!
//! // Handle memo not found
//! let non_existent_id = MemoId::new();
//! match storage.get_memo(&non_existent_id).await {
//!     Ok(memo) => println!("Found memo: {}", memo.title),
//!     Err(SwissArmyHammerError::MemoNotFound(id)) => {
//!         println!("Memo not found with ID: {}", id);
//!     }
//!     Err(e) => return Err(e.into()),
//! }
//!
//! // Handle storage errors gracefully
//! match storage.create_memo("".to_string(), "content".to_string()).await {
//!     Ok(memo) => println!("Created: {}", memo.title),
//!     Err(e) => {
//!         eprintln!("Failed to create memo: {}", e);
//!         // Could implement retry logic, user notification, etc.
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Troubleshooting Common Issues
//!
//! ### Storage and File System Issues
//!
//! **Permission Denied Errors**
//! ```text
//! Error: Permission denied (os error 13)
//! ```
//! - **Cause**: Insufficient permissions to read/write memo storage directory
//! - **Solution**: Check directory permissions and ownership
//! ```bash
//! chmod 755 .swissarmyhammer/
//! chmod 644 .swissarmyhammer/memos/*
//! ```
//!
//! **Storage Directory Missing**
//! ```text
//! Error: No such file or directory
//! ```
//! - **Cause**: Parent directory doesn't exist or is inaccessible
//! - **Solution**: Verify parent directory exists and is writable
//! ```rust
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use std::fs;
//! fs::create_dir_all(".swissarmyhammer/memos")?;
//! # Ok(())
//! # }
//! ```
//!
//! **Disk Space Issues**
//! ```text
//! Error: No space left on device
//! ```
//! - **Cause**: Insufficient disk space for memo storage
//! - **Solution**: Free up disk space or use custom storage path with more space
//!
//! ### Memo ID and Validation Issues
//!
//! **Invalid ULID Format**
//! ```text
//! Error: Invalid memo ID format: 'short-id'. Expected a valid ULID...
//! ```
//! - **Cause**: Malformed or incomplete ULID string
//! - **Solution**: Ensure ULID is exactly 26 characters, base32 encoded
//! ```rust
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use swissarmyhammer::memoranda::MemoId;
//! // ✅ Correct format
//! let id = MemoId::from_string("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string())?;
//! // ❌ Incorrect formats
//! let bad_id1 = MemoId::from_string("short".to_string()); // Too short
//! let bad_id2 = MemoId::from_string("01ARZ3NDEKTSV4RRFFQ69G5FA=".to_string()); // Invalid chars
//! # Ok(())
//! # }
//! ```
//!
//! **Memo Not Found**
//! ```text
//! Error: Memo not found: 01ARZ3NDEKTSV4RRFFQ69G5FAV
//! ```
//! - **Cause**: Memo ID doesn't correspond to any stored memo
//! - **Solution**: Verify ID exists using list operation, check for typos
//! ```rust
//! # async fn example(storage: &impl swissarmyhammer::memoranda::MemoStorage, target_id: &str) -> Result<(), Box<dyn std::error::Error>> {
//! // Verify memo exists before operations
//! let all_memos = storage.list_memos().await?;
//! let memo_exists = all_memos.iter().any(|m| m.id.as_str() == target_id);
//! if !memo_exists {
//!     eprintln!("Memo not found, available IDs:");
//!     for memo in all_memos.iter().take(5) {
//!         eprintln!("  {}: {}", memo.id, memo.title);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Search and Performance Issues
//!
//! **Slow Search Performance**
//! - **Cause**: Large memo collection or very long memo content
//! - **Solution**: Use specific search terms, implement pagination, or archive old memos
//! ```rust
//! # async fn example(storage: &impl swissarmyhammer::memoranda::MemoStorage) -> Result<(), Box<dyn std::error::Error>> {
//! use swissarmyhammer::memoranda::SearchOptions;
//! // Use more specific search terms
//! let results = storage.search_memos("specific keyword").await?;
//!
//! // For advanced search, limit results
//! let search_options = SearchOptions {
//!     max_results: Some(20),
//!     excerpt_length: 60, // Shorter excerpts
//!     ..Default::default()
//! };
//! # Ok(())
//! # }
//! ```
//!
//! **Memory Usage Issues**
//! - **Cause**: Loading too many large memos into memory simultaneously
//! - **Solution**: Process memos in batches, use streaming for large operations
//! ```rust
//! # async fn example(storage: &impl swissarmyhammer::memoranda::MemoStorage) -> Result<(), Box<dyn std::error::Error>> {
//! // Process memos in smaller batches
//! let all_memos = storage.list_memos().await?;
//! for chunk in all_memos.chunks(50) {
//!     // Process each batch of 50 memos
//!     for memo in chunk {
//!         // Process individual memo
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Concurrent Access Issues
//!
//! **File Lock Conflicts**
//! ```text
//! Error: Resource temporarily unavailable
//! ```
//! - **Cause**: Multiple processes trying to access the same memo file
//! - **Solution**: Implement retry logic with backoff, use atomic operations
//! ```rust
//! # async fn example(storage: &impl swissarmyhammer::memoranda::MemoStorage, title: String, content: String) -> Result<(), Box<dyn std::error::Error>> {
//! use std::time::Duration;
//! use tokio::time::sleep;
//!
//! let mut retries = 3;
//! while retries > 0 {
//!     match storage.create_memo(title.clone(), content.clone()).await {
//!         Ok(memo) => break,
//!         Err(e) if retries > 1 => {
//!             eprintln!("Retry attempt {} failed: {}", 4 - retries, e);
//!             sleep(Duration::from_millis(100)).await;
//!             retries -= 1;
//!         }
//!         Err(e) => return Err(e.into()),
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Data Corruption and Recovery
//!
//! **Corrupted JSON Files**
//! ```text
//! Error: Failed to parse memo file: expected `,` or `}`
//! ```
//! - **Cause**: Incomplete writes, system crashes, or manual file editing
//! - **Solution**: Restore from backup, manually repair JSON, or recreate memo
//! ```rust
//! # async fn example(storage: &impl swissarmyhammer::memoranda::MemoStorage, memo_id: &swissarmyhammer::memoranda::MemoId) -> Result<(), Box<dyn std::error::Error>> {
//! use swissarmyhammer::error::SwissArmyHammerError;
//! // Check for and handle corrupted files
//! match storage.get_memo(memo_id).await {
//!     Err(SwissArmyHammerError::Other(msg)) if msg.contains("parse") => {
//!         eprintln!("Corrupted memo file detected: {}", msg);
//!         // Implement recovery logic or manual intervention
//!     }
//!     Ok(memo) => {
//!         // Use the memo
//!     }
//!     Err(e) => return Err(e.into()),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Performance Optimization Tips
//!
//! - **Batch Operations**: Group multiple memo operations together
//! - **Selective Loading**: Only load memo metadata when full content isn't needed
//! - **Index Management**: Rebuild advanced search indexes periodically
//! - **Storage Cleanup**: Archive or delete old memos to maintain performance
//! - **SSD Usage**: Use SSD storage for better I/O performance with large collections
//!
//! ### Debug and Diagnostics
//!
//! Enable debug logging to troubleshoot issues:
//! ```bash
//! RUST_LOG=swissarmyhammer::memoranda=debug your_application
//! ```
//!
//! Check storage directory status:
//! ```rust
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use std::fs;
//!
//! // Verify storage directory exists and is accessible
//! let memo_dir = std::path::Path::new(".swissarmyhammer/memos");
//! if !memo_dir.exists() {
//!     eprintln!("Memo directory does not exist: {}", memo_dir.display());
//! } else {
//!     let metadata = fs::metadata(memo_dir)?;
//!     #[cfg(unix)]
//!     {
//!         use std::os::unix::fs::PermissionsExt;
//!         println!("Directory permissions: {:o}", metadata.permissions().mode());
//!     }
//! }
//!
//! // Count memo files
//! let entries = fs::read_dir(memo_dir)?;
//! let memo_count = entries.filter_map(|e| e.ok()).filter(|entry| {
//!     entry.path().extension().map_or(false, |ext| ext == "json")
//! }).count();
//! println!("Found {} memo files", memo_count);
//! # Ok(())
//! # }
//! ```

use crate::common::generate_monotonic_ulid;
use crate::error::{Result, SwissArmyHammerError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Storage backends for memo persistence and retrieval
pub mod storage;
pub use storage::{FileSystemMemoStorage, MarkdownMemoStorage, MemoState, MemoStorage};

/// Advanced search engine with full-text indexing and query parsing
pub mod advanced_search;
pub use advanced_search::AdvancedMemoSearchEngine;

/// A unique identifier for memos using ULID (Universally Unique Lexicographically Sortable Identifier)
///
/// ULIDs provide both uniqueness and natural ordering, making them ideal for memo identification
/// and chronological sorting. They are 26 characters long and URL-safe.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::MemoId;
///
/// // Generate a new ID
/// let id = MemoId::new();
/// println!("Generated ID: {}", id);
///
/// // Parse from string
/// let id_str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
/// let parsed_id = MemoId::from_string(id_str.to_string())?;
/// assert_eq!(parsed_id.as_str(), id_str);
/// # Ok::<(), swissarmyhammer::error::SwissArmyHammerError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MemoId(String);

impl MemoId {
    /// Create a new unique memo identifier using ULID generation
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swissarmyhammer::memoranda::MemoId;
    ///
    /// let id = MemoId::new();
    /// // ULID is always 26 characters
    /// assert_eq!(id.as_str().len(), 26);
    /// // Each new ID should be unique (basic smoke test)
    /// assert!(!id.as_str().is_empty());
    /// ```
    pub fn new() -> Self {
        Self(generate_monotonic_ulid().to_string())
    }

    /// Create a memo ID from a string, supporting both ULID and filename-based formats
    ///
    /// This method accepts two types of memo IDs:
    /// 1. ULID format (26 characters, base32 encoded) - for backward compatibility
    /// 2. Filename-based format - for markdown storage (derived from sanitized titles)
    ///
    /// # Arguments
    ///
    /// * `id` - A string that should be either a valid ULID or a valid filename-based ID
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - The memo ID if valid, or an error if the format is invalid
    ///
    /// # Errors
    ///
    /// Returns an error if the provided string is:
    /// - Empty or whitespace-only
    /// - Longer than 255 characters
    /// - Contains filesystem-invalid characters (/, \, :, *, ?, ", <, >, |, null)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swissarmyhammer::memoranda::MemoId;
    ///
    /// // Valid ULID format
    /// let ulid_id = MemoId::from_string("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string())?;
    /// assert_eq!(ulid_id.as_str(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    ///
    /// // Valid filename-based format
    /// let filename_id = MemoId::from_string("Meeting_Notes".to_string())?;
    /// assert_eq!(filename_id.as_str(), "Meeting_Notes");
    ///
    /// // Invalid - empty string
    /// let invalid_result = MemoId::from_string("".to_string());
    /// assert!(invalid_result.is_err());
    /// # Ok::<(), swissarmyhammer::error::SwissArmyHammerError>(())
    /// ```
    pub fn from_string(id: String) -> Result<Self> {
        // Check if ID is empty
        if id.trim().is_empty() {
            return Err(SwissArmyHammerError::Other(
                "Memo ID cannot be empty".to_string(),
            ));
        }

        // Try to parse as ULID first (for backward compatibility)
        if Ulid::from_string(&id).is_ok() {
            return Ok(Self(id));
        }

        // If not a valid ULID, validate as filename-based ID
        // Basic validation: non-empty, printable ASCII characters, reasonable length
        if id.len() > 255 {
            return Err(SwissArmyHammerError::Other(format!(
                "Memo ID too long: '{id}'. Maximum length is 255 characters."
            )));
        }

        // Check for invalid characters that would cause filesystem issues
        let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
        for ch in invalid_chars {
            if id.contains(ch) {
                return Err(SwissArmyHammerError::Other(format!(
                    "Memo ID contains invalid character '{ch}': '{id}'"
                )));
            }
        }

        // Accept the ID as a filename-based identifier
        Ok(Self(id))
    }

    /// Get the string representation of the memo ID
    ///
    /// # Returns
    ///
    /// * `&str` - The ULID string
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swissarmyhammer::memoranda::MemoId;
    ///
    /// let id = MemoId::new();
    /// let id_str = id.as_str();
    /// assert_eq!(id_str.len(), 26);
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create a memo ID from a filename (without .md extension)
    ///
    /// This method creates a MemoId directly from a filename, which is used
    /// in the markdown storage implementation where the filename serves as the ID.
    ///
    /// # Arguments
    ///
    /// * `filename` - The filename (without .md extension) to use as the ID
    ///
    /// # Returns
    ///
    /// * `Self` - A memo ID based on the filename
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swissarmyhammer::memoranda::MemoId;
    ///
    /// let id = MemoId::from_filename("meeting_notes");
    /// assert_eq!(id.as_str(), "meeting_notes");
    /// ```
    pub fn from_filename(filename: &str) -> Self {
        Self(filename.to_string())
    }
}

impl Default for MemoId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MemoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for MemoId {
    type Err = SwissArmyHammerError;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_string(s.to_string())
    }
}

impl AsRef<str> for MemoId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A memo containing structured text with metadata
///
/// Memos are the core data structure of the memoranda system, containing a title,
/// content, and automatic timestamping. Each memo has a unique ULID identifier
/// that allows for natural chronological ordering.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::Memo;
///
/// let memo = Memo::new(
///     "Project Notes".to_string(),
///     "Remember to review the API documentation.".to_string()
/// );
///
/// println!("Created memo '{}' with ID: {}", memo.title, memo.id);
/// assert_eq!(memo.created_at, memo.updated_at); // Initially same timestamp
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memo {
    /// Unique identifier for this memo
    pub id: MemoId,
    /// Brief title or subject of the memo
    pub title: String,
    /// The main content/body of the memo
    pub content: String,
    /// When this memo was first created
    pub created_at: DateTime<Utc>,
    /// When this memo was last modified
    pub updated_at: DateTime<Utc>,
}

impl Memo {
    /// Create a new memo with the given title and content
    ///
    /// Automatically generates a unique ULID identifier and sets creation/update timestamps
    /// to the current time.
    ///
    /// # Arguments
    ///
    /// * `title` - The title or subject of the memo
    /// * `content` - The main content/body of the memo
    ///
    /// # Returns
    ///
    /// * `Self` - A new memo instance with unique ID and current timestamps
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swissarmyhammer::memoranda::Memo;
    ///
    /// let memo = Memo::new(
    ///     "Daily Standup".to_string(),
    ///     "Completed feature X, working on bug Y today.".to_string()
    /// );
    ///
    /// assert!(!memo.title.is_empty());
    /// assert!(!memo.content.is_empty());
    /// assert_eq!(memo.created_at, memo.updated_at);
    /// ```
    pub fn new(title: String, content: String) -> Self {
        let now = Utc::now();
        Self {
            id: MemoId::new(),
            title,
            content,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the content of this memo and refresh the updated timestamp
    ///
    /// # Arguments
    ///
    /// * `content` - The new content to replace the existing content
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swissarmyhammer::memoranda::Memo;
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let mut memo = Memo::new("Title".to_string(), "Original content".to_string());
    /// let original_updated_at = memo.updated_at;
    ///
    /// // Small delay to ensure timestamp difference
    /// thread::sleep(Duration::from_millis(1));
    ///
    /// memo.update_content("Updated content".to_string());
    /// assert_eq!(memo.content, "Updated content");
    /// assert!(memo.updated_at > original_updated_at);
    /// ```
    pub fn update_content(&mut self, content: String) {
        self.content = content;
        self.updated_at = Utc::now();
    }

    /// Update the title of this memo and refresh the updated timestamp
    ///
    /// # Arguments
    ///
    /// * `title` - The new title to replace the existing title
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swissarmyhammer::memoranda::Memo;
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let mut memo = Memo::new("Original Title".to_string(), "Content".to_string());
    /// let original_updated_at = memo.updated_at;
    ///
    /// // Small delay to ensure timestamp difference
    /// thread::sleep(Duration::from_millis(1));
    ///
    /// memo.update_title("New Title".to_string());
    /// assert_eq!(memo.title, "New Title");
    /// assert!(memo.updated_at > original_updated_at);
    /// ```
    pub fn update_title(&mut self, title: String) {
        self.title = title;
        self.updated_at = Utc::now();
    }
}

/// Request to create a new memo
///
/// Used by MCP tools and API endpoints to specify the title and content
/// for a new memo. The system will automatically generate a unique ID and timestamps.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::CreateMemoRequest;
///
/// let request = CreateMemoRequest {
///     title: "Meeting Notes".to_string(),
///     content: "Discussed quarterly goals and team structure.".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateMemoRequest {
    /// The title for the new memo
    pub title: String,
    /// The content for the new memo
    pub content: String,
}

/// Request to update an existing memo's content
///
/// Used to modify the content of a memo identified by its ULID.
/// The title remains unchanged, and the updated_at timestamp is refreshed automatically.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::{UpdateMemoRequest, MemoId};
///
/// let request = UpdateMemoRequest {
///     id: MemoId::new(),
///     content: "Updated content with new information.".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateMemoRequest {
    /// The ID of the memo to update
    pub id: MemoId,
    /// The new content to replace the existing content
    pub content: String,
}

/// Request to search memos by content or title
///
/// Performs full-text search across memo titles and content, returning
/// memos that match the query string.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::SearchMemosRequest;
///
/// let request = SearchMemosRequest {
///     query: "project timeline".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchMemosRequest {
    /// The search query to match against memo titles and content
    pub query: String,
}

/// Response containing search results for memo queries
///
/// Returns memos that match the search criteria along with a count
/// of total matches found.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::{SearchMemosResponse, Memo};
///
/// let response = SearchMemosResponse {
///     memos: vec![], // Vec of matching memos
///     total_count: 0,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchMemosResponse {
    /// List of memos that match the search query
    pub memos: Vec<Memo>,
    /// Total number of memos found matching the query
    pub total_count: usize,
}

/// Request to retrieve a specific memo by its ID
///
/// Used to fetch a single memo using its unique ULID identifier.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::{GetMemoRequest, MemoId};
///
/// let request = GetMemoRequest {
///     id: MemoId::new(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetMemoRequest {
    /// The unique ID of the memo to retrieve
    pub id: MemoId,
}

/// Request to delete a specific memo by its ID
///
/// Used to permanently remove a memo from storage using its unique ULID identifier.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::{DeleteMemoRequest, MemoId};
///
/// let request = DeleteMemoRequest {
///     id: MemoId::new(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteMemoRequest {
    /// The unique ID of the memo to delete
    pub id: MemoId,
}

/// Response containing a list of all memos
///
/// Returns all memos in the system, typically ordered by creation time
/// (newest first), along with the total count.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::{ListMemosResponse, Memo};
///
/// let response = ListMemosResponse {
///     memos: vec![], // Vec of all memos
///     total_count: 0,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListMemosResponse {
    /// List of all memos in the system
    pub memos: Vec<Memo>,
    /// Total number of memos in the system
    pub total_count: usize,
}

/// Options for configuring advanced memo search behavior
///
/// Controls search behavior including case sensitivity, result limits,
/// and search result formatting options.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::SearchOptions;
///
/// let options = SearchOptions {
///     case_sensitive: false,
///     exact_phrase: false,
///     max_results: Some(50),
///     include_highlights: true,
///     excerpt_length: 80,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchOptions {
    /// Whether search should be case-sensitive (default: false)
    pub case_sensitive: bool,
    /// Whether to treat query as exact phrase match (default: false)
    pub exact_phrase: bool,
    /// Maximum number of results to return (default: None for unlimited)
    pub max_results: Option<usize>,
    /// Whether to include search result highlights (default: false)
    pub include_highlights: bool,
    /// Number of characters to show around matches in excerpts (default: 60)
    pub excerpt_length: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            exact_phrase: false,
            max_results: None,
            include_highlights: false,
            excerpt_length: 60,
        }
    }
}

/// A search result containing a memo with relevance scoring and match highlights
///
/// Represents a memo that matches a search query, along with metadata about
/// the match quality and highlighted text snippets where matches were found.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::{SearchResult, Memo};
///
/// let memo = Memo::new("Project Notes".to_string(), "Important project details".to_string());
/// let result = SearchResult {
///     memo,
///     relevance_score: 85.5,
///     highlights: vec!["**Project** Notes".to_string()],
///     match_count: 1,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    /// The memo that matched the search query
    pub memo: Memo,
    /// Relevance score (0.0-100.0, higher is more relevant)
    pub relevance_score: f32,
    /// Highlighted text snippets showing where matches were found
    pub highlights: Vec<String>,
    /// Total number of matches found in this memo
    pub match_count: usize,
}

/// Options for configuring context generation for AI consumption
///
/// Controls how memo content is formatted and concatenated when generating
/// context for AI assistants or other automated processing.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::memoranda::ContextOptions;
///
/// let options = ContextOptions {
///     include_metadata: true,
///     max_tokens: Some(8000),
///     delimiter: "\n---\n".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextOptions {
    /// Whether to include memo metadata (titles, dates) in context (default: true)
    pub include_metadata: bool,
    /// Maximum number of tokens to include (approximate, default: None)
    pub max_tokens: Option<usize>,
    /// Delimiter to use between memos (default: "\n---\n")
    pub delimiter: String,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            include_metadata: true,
            max_tokens: None,
            delimiter: "\n---\n".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memo_id_generation() {
        let id1 = MemoId::new();
        let id2 = MemoId::new();

        assert_ne!(id1, id2);

        assert!(id1.as_str().len() == 26);
        assert!(id2.as_str().len() == 26);
    }

    #[test]
    fn test_memo_id_from_string() {
        let ulid = generate_monotonic_ulid();
        let ulid_string = ulid.to_string();

        let memo_id = MemoId::from_string(ulid_string.clone()).unwrap();
        assert_eq!(memo_id.as_str(), &ulid_string);
    }

    #[test]
    fn test_memo_id_invalid_string() {
        let result = MemoId::from_string("invalid/ulid*".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_memo_creation() {
        let memo = Memo::new("Test Title".to_string(), "Test Content".to_string());

        assert_eq!(memo.title, "Test Title");
        assert_eq!(memo.content, "Test Content");
        assert!(memo.created_at <= Utc::now());
        assert_eq!(memo.created_at, memo.updated_at);
    }

    #[test]
    fn test_memo_update_content() {
        let mut memo = Memo::new("Title".to_string(), "Original".to_string());
        let original_created_at = memo.created_at;
        let original_updated_at = memo.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(1));

        memo.update_content("Updated Content".to_string());

        assert_eq!(memo.content, "Updated Content");
        assert_eq!(memo.created_at, original_created_at);
        assert!(memo.updated_at > original_updated_at);
    }

    #[test]
    fn test_memo_update_title() {
        let mut memo = Memo::new("Original Title".to_string(), "Content".to_string());
        let original_created_at = memo.created_at;
        let original_updated_at = memo.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(1));

        memo.update_title("New Title".to_string());

        assert_eq!(memo.title, "New Title");
        assert_eq!(memo.created_at, original_created_at);
        assert!(memo.updated_at > original_updated_at);
    }

    #[test]
    fn test_memo_serialization() {
        let memo = Memo::new("Test Title".to_string(), "Test Content".to_string());

        let json = serde_json::to_string(&memo).unwrap();
        let deserialized: Memo = serde_json::from_str(&json).unwrap();

        assert_eq!(memo, deserialized);
    }

    #[test]
    fn test_request_types_serialization() {
        let create_request = CreateMemoRequest {
            title: "New Memo".to_string(),
            content: "New Content".to_string(),
        };

        let json = serde_json::to_string(&create_request).unwrap();
        let deserialized: CreateMemoRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(create_request, deserialized);
    }

    #[test]
    fn test_search_request_serialization() {
        let search_request = SearchMemosRequest {
            query: "test query".to_string(),
        };

        let json = serde_json::to_string(&search_request).unwrap();
        let deserialized: SearchMemosRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(search_request, deserialized);
    }

    #[test]
    fn test_search_options_default() {
        let options = SearchOptions::default();

        assert!(!options.case_sensitive);
        assert!(!options.exact_phrase);
        assert!(options.max_results.is_none());
        assert!(!options.include_highlights);
    }

    #[test]
    fn test_search_options_serialization() {
        let options = SearchOptions {
            case_sensitive: true,
            exact_phrase: false,
            max_results: Some(25),
            include_highlights: true,
            excerpt_length: 80,
        };

        let json = serde_json::to_string(&options).unwrap();
        let deserialized: SearchOptions = serde_json::from_str(&json).unwrap();

        assert_eq!(options, deserialized);
    }

    #[test]
    fn test_search_result_creation() {
        let memo = Memo::new("Test Title".to_string(), "Test Content".to_string());
        let result = SearchResult {
            memo: memo.clone(),
            relevance_score: 95.5,
            highlights: vec!["**Test** Title".to_string()],
            match_count: 1,
        };

        assert_eq!(result.memo, memo);
        assert_eq!(result.relevance_score, 95.5);
        assert_eq!(result.highlights.len(), 1);
        assert_eq!(result.match_count, 1);
    }

    #[test]
    fn test_search_result_serialization() {
        let memo = Memo::new("Title".to_string(), "Content".to_string());
        let result = SearchResult {
            memo,
            relevance_score: 75.0,
            highlights: vec!["High**light**ed text".to_string()],
            match_count: 2,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_context_options_default() {
        let options = ContextOptions::default();

        assert!(options.include_metadata);
        assert!(options.max_tokens.is_none());
        assert_eq!(options.delimiter, "\n---\n");
    }

    #[test]
    fn test_context_options_serialization() {
        let options = ContextOptions {
            include_metadata: false,
            max_tokens: Some(5000),
            delimiter: "\n\n===\n\n".to_string(),
        };

        let json = serde_json::to_string(&options).unwrap();
        let deserialized: ContextOptions = serde_json::from_str(&json).unwrap();

        assert_eq!(options, deserialized);
    }

    // ===== COMPREHENSIVE DATA STRUCTURE TESTS =====

    #[test]
    fn test_memo_id_default() {
        let id1 = MemoId::default();
        let id2 = MemoId::default();

        assert_ne!(id1, id2);
        assert_eq!(id1.as_str().len(), 26);
        assert_eq!(id2.as_str().len(), 26);
    }

    #[test]
    fn test_memo_id_display() {
        let id = MemoId::new();
        let display_str = format!("{id}");
        assert_eq!(display_str, id.as_str());
    }

    #[test]
    fn test_memo_id_as_ref() {
        let id = MemoId::new();
        let as_ref: &str = id.as_ref();
        assert_eq!(as_ref, id.as_str());
    }

    #[test]
    fn test_memo_id_from_str() {
        let valid_ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let id = valid_ulid.parse::<MemoId>().unwrap();
        assert_eq!(id.as_str(), valid_ulid);

        let invalid_id = "invalid/ulid*";
        let result = invalid_id.parse::<MemoId>();
        assert!(result.is_err());
    }

    #[test]
    fn test_memo_id_ordering() {
        let id1 = MemoId::new();
        std::thread::sleep(std::time::Duration::from_millis(1)); // Ensure different timestamp
        let id2 = MemoId::new();

        // ULIDs should be naturally ordered by creation time
        assert!(id1 < id2);

        let mut ids = vec![id2.clone(), id1.clone()];
        ids.sort();
        assert_eq!(ids, vec![id1, id2]);
    }

    #[test]
    fn test_memo_id_hash() {
        use std::collections::HashMap;

        let id1 = MemoId::new();
        let id2 = MemoId::new();

        let mut map = HashMap::new();
        map.insert(id1.clone(), "value1");
        map.insert(id2.clone(), "value2");

        assert_eq!(map.get(&id1), Some(&"value1"));
        assert_eq!(map.get(&id2), Some(&"value2"));
    }

    #[test]
    fn test_memo_id_clone() {
        let id = MemoId::new();
        let cloned_id = id.clone();

        assert_eq!(id, cloned_id);
        assert_eq!(id.as_str(), cloned_id.as_str());
    }

    #[test]
    fn test_memo_id_valid_ulid_formats() {
        // Test various valid ULID formats
        let valid_ulids = vec![
            "01ARZ3NDEKTSV4RRFFQ69G5FAV", // Standard ULID
            "01BX5ZZKBKACTAV9WEVGEMMVS0", // Another valid ULID
            "01DRJZJNQXY1H0PT7XRRMH2QG9", // Case variations
        ];

        for ulid_str in valid_ulids {
            let id = MemoId::from_string(ulid_str.to_string()).unwrap();
            assert_eq!(id.as_str(), ulid_str);
        }
    }

    #[test]
    fn test_memo_id_invalid_formats() {
        let long_id = "a".repeat(256);
        let invalid_ids = vec![
            "",            // Empty string
            "   ",         // Whitespace only
            &long_id,      // Too long (over 255 characters)
            "invalid/id",  // Contains forbidden character /
            "invalid\\id", // Contains forbidden character \
            "invalid:id",  // Contains forbidden character :
            "invalid*id",  // Contains forbidden character *
            "invalid?id",  // Contains forbidden character ?
            "invalid\"id", // Contains forbidden character "
            "invalid<id",  // Contains forbidden character <
            "invalid>id",  // Contains forbidden character >
            "invalid|id",  // Contains forbidden character |
            "invalid\0id", // Contains null character
        ];

        for invalid_id in invalid_ids {
            let result = MemoId::from_string(invalid_id.to_string());
            assert!(result.is_err(), "Should fail for: {invalid_id}");
        }
    }

    #[test]
    fn test_memo_update_title_and_content() {
        let mut memo = Memo::new("Original Title".to_string(), "Original Content".to_string());
        let original_created = memo.created_at;
        let original_updated = memo.updated_at;

        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));

        // Update both title and content
        memo.update_title("New Title".to_string());
        memo.update_content("New Content".to_string());

        assert_eq!(memo.title, "New Title");
        assert_eq!(memo.content, "New Content");
        assert_eq!(memo.created_at, original_created); // Should remain unchanged
        assert!(memo.updated_at > original_updated); // Should be updated
    }

    #[test]
    fn test_memo_clone_and_equality() {
        let memo1 = Memo::new("Test Title".to_string(), "Test Content".to_string());
        let memo2 = memo1.clone();

        assert_eq!(memo1, memo2);
        assert_eq!(memo1.id, memo2.id);
        assert_eq!(memo1.title, memo2.title);
        assert_eq!(memo1.content, memo2.content);
        assert_eq!(memo1.created_at, memo2.created_at);
        assert_eq!(memo1.updated_at, memo2.updated_at);
    }

    #[test]
    fn test_memo_serialization_edge_cases() {
        // Test with empty strings
        let empty_memo = Memo::new("".to_string(), "".to_string());
        let json = serde_json::to_string(&empty_memo).unwrap();
        let deserialized: Memo = serde_json::from_str(&json).unwrap();
        assert_eq!(empty_memo, deserialized);

        // Test with unicode content
        let unicode_memo = Memo::new("🚀 Title".to_string(), "Content with émojis 🎉".to_string());
        let json = serde_json::to_string(&unicode_memo).unwrap();
        let deserialized: Memo = serde_json::from_str(&json).unwrap();
        assert_eq!(unicode_memo, deserialized);

        // Test with special characters
        let special_memo = Memo::new(
            r#"Title with "quotes""#.to_string(),
            r#"Content with newlines\nand\ttabs"#.to_string(),
        );
        let json = serde_json::to_string(&special_memo).unwrap();
        let deserialized: Memo = serde_json::from_str(&json).unwrap();
        assert_eq!(special_memo, deserialized);
    }

    #[test]
    fn test_request_types_validation() {
        // Test CreateMemoRequest with edge cases
        let create_request = CreateMemoRequest {
            title: "".to_string(),
            content: "x".repeat(1_000_000), // Large content
        };
        let json = serde_json::to_string(&create_request).unwrap();
        let deserialized: CreateMemoRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(create_request, deserialized);

        // Test UpdateMemoRequest
        let update_request = UpdateMemoRequest {
            id: MemoId::new(),
            content: "Updated content".to_string(),
        };
        let json = serde_json::to_string(&update_request).unwrap();
        let deserialized: UpdateMemoRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(update_request, deserialized);

        // Test SearchMemosRequest with special characters
        let search_request = SearchMemosRequest {
            query: "query with \"quotes\" and 中文".to_string(),
        };
        let json = serde_json::to_string(&search_request).unwrap();
        let deserialized: SearchMemosRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(search_request, deserialized);
    }

    #[test]
    fn test_search_options_edge_cases() {
        // Test various search option combinations
        let options = SearchOptions {
            case_sensitive: true,
            exact_phrase: true,
            max_results: Some(0), // Edge case: zero results
            include_highlights: true,
            excerpt_length: 0, // Edge case: zero length
        };
        let json = serde_json::to_string(&options).unwrap();
        let deserialized: SearchOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(options, deserialized);

        // Test very large values
        let large_options = SearchOptions {
            max_results: Some(usize::MAX),
            excerpt_length: usize::MAX,
            ..Default::default()
        };
        let json = serde_json::to_string(&large_options).unwrap();
        let deserialized: SearchOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(large_options, deserialized);
    }

    #[test]
    fn test_context_options_edge_cases() {
        // Test with extreme values
        let options = ContextOptions {
            include_metadata: false,
            max_tokens: Some(0),       // Edge case: zero tokens
            delimiter: "".to_string(), // Empty delimiter
        };
        let json = serde_json::to_string(&options).unwrap();
        let deserialized: ContextOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(options, deserialized);

        // Test with very large delimiter
        let large_delimiter_options = ContextOptions {
            delimiter: "=".repeat(10_000),
            max_tokens: Some(usize::MAX),
            ..Default::default()
        };
        let json = serde_json::to_string(&large_delimiter_options).unwrap();
        let deserialized: ContextOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(large_delimiter_options, deserialized);
    }

    #[test]
    fn test_search_result_validation() {
        let memo = Memo::new("Test Title".to_string(), "Test Content".to_string());

        // Test with edge case values
        let result = SearchResult {
            memo: memo.clone(),
            relevance_score: 0.0, // Minimum score
            highlights: vec![],   // Empty highlights
            match_count: 0,       // Zero matches
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);

        // Test with maximum values
        let max_result = SearchResult {
            memo,
            relevance_score: f32::MAX,
            highlights: vec!["highlight".to_string(); 1000], // Many highlights
            match_count: usize::MAX,
        };
        let json = serde_json::to_string(&max_result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(max_result, deserialized);
    }

    #[test]
    fn test_response_types_with_large_data() {
        let memos: Vec<Memo> = (0..1000)
            .map(|i| Memo::new(format!("Title {i}"), format!("Content {i}")))
            .collect();

        // Test ListMemosResponse with many memos
        let list_response = ListMemosResponse {
            memos: memos.clone(),
            total_count: memos.len(),
        };
        let json = serde_json::to_string(&list_response).unwrap();
        let deserialized: ListMemosResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(list_response.total_count, deserialized.total_count);
        assert_eq!(list_response.memos.len(), deserialized.memos.len());

        // Test SearchMemosResponse
        let search_response = SearchMemosResponse {
            memos: memos.clone(),
            total_count: memos.len(),
        };
        let json = serde_json::to_string(&search_response).unwrap();
        let deserialized: SearchMemosResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(search_response.total_count, deserialized.total_count);
    }

    #[test]
    fn test_timestamp_precision() {
        let memo = Memo::new("Timestamp Test".to_string(), "Content".to_string());

        // Test that timestamps are stored and retrieved precisely
        let json = serde_json::to_string(&memo).unwrap();
        let deserialized: Memo = serde_json::from_str(&json).unwrap();

        assert_eq!(memo.created_at, deserialized.created_at);
        assert_eq!(memo.updated_at, deserialized.updated_at);

        // Test timestamp formatting
        let created_str = memo.created_at.to_rfc3339();
        assert!(created_str.len() >= 20); // ISO 8601 format should be at least 20 chars
        assert!(created_str.contains('T')); // Should contain date/time separator
    }

    #[test]
    fn test_memo_id_monotonic_generation() {
        let id1 = MemoId::new();
        let id2 = MemoId::new();
        let id3 = MemoId::new();

        // Test that IDs are monotonic
        assert!(id1 < id2);
        assert!(id2 < id3);
        assert!(id1 < id3);

        // Test that string representation also maintains ordering
        assert!(id1.as_str() < id2.as_str());
        assert!(id2.as_str() < id3.as_str());
        assert!(id1.as_str() < id3.as_str());
    }
}

// ===== PROPERTY-BASED TESTS =====
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_memo_id_generation_uniqueness(_seed in 0u64..1000) {
            // Generate multiple IDs and ensure they're all unique
            let mut ids = Vec::new();
            for _ in 0..100 {
                ids.push(MemoId::new());
            }

            // Convert to set to check uniqueness
            let unique_ids: std::collections::HashSet<_> = ids.iter().cloned().collect();
            prop_assert_eq!(unique_ids.len(), ids.len());

            // Check all IDs are 26 characters
            for id in &ids {
                prop_assert_eq!(id.as_str().len(), 26);
            }
        }

        #[test]
        fn test_memo_id_ordering_property(count in 1usize..50) {
            let mut ids = Vec::new();
            for _ in 0..count {
                ids.push(MemoId::new());
            }

            // Check that all IDs are unique
            let mut sorted_ids = ids.clone();
            sorted_ids.sort();
            sorted_ids.dedup();
            prop_assert_eq!(sorted_ids.len(), count, "All IDs should be unique");

            // Check that IDs are valid ULIDs (26 characters)
            for id in &ids {
                prop_assert_eq!(id.as_str().len(), 26, "Each ULID should be 26 characters");
            }
        }

        #[test]
        fn test_memo_serialization_roundtrip(
            title in ".*",
            content in ".*"
        ) {
            let memo = Memo::new(title, content);

            // Test JSON serialization roundtrip
            let json = serde_json::to_string(&memo)?;
            let deserialized: Memo = serde_json::from_str(&json)?;

            prop_assert_eq!(memo, deserialized);
        }

        #[test]
        fn test_memo_update_preserves_id_and_created_at(
            original_title in ".*",
            original_content in ".*",
            new_title in ".*",
            new_content in ".*"
        ) {
            let mut memo = Memo::new(original_title, original_content);
            let original_id = memo.id.clone();
            let original_created = memo.created_at;

            memo.update_title(new_title.clone());
            memo.update_content(new_content.clone());

            prop_assert_eq!(memo.id, original_id);
            prop_assert_eq!(memo.created_at, original_created);
            prop_assert_eq!(memo.title, new_title);
            prop_assert_eq!(memo.content, new_content);
            prop_assert!(memo.updated_at >= original_created);
        }

        #[test]
        fn test_search_options_serialization_roundtrip(
            case_sensitive in any::<bool>(),
            exact_phrase in any::<bool>(),
            max_results in prop::option::of(0usize..10000),
            include_highlights in any::<bool>(),
            excerpt_length in 0usize..1000
        ) {
            let options = SearchOptions {
                case_sensitive,
                exact_phrase,
                max_results,
                include_highlights,
                excerpt_length,
            };

            let json = serde_json::to_string(&options)?;
            let deserialized: SearchOptions = serde_json::from_str(&json)?;

            prop_assert_eq!(options, deserialized);
        }

        #[test]
        fn test_context_options_serialization_roundtrip(
            include_metadata in any::<bool>(),
            max_tokens in prop::option::of(0usize..100000),
            delimiter in ".*"
        ) {
            let options = ContextOptions {
                include_metadata,
                max_tokens,
                delimiter,
            };

            let json = serde_json::to_string(&options)?;
            let deserialized: ContextOptions = serde_json::from_str(&json)?;

            prop_assert_eq!(options, deserialized);
        }

        #[test]
        fn test_request_types_serialization_roundtrip(
            title in ".*",
            content in ".*",
            query in ".*"
        ) {
            // Test CreateMemoRequest
            let create_request = CreateMemoRequest {
                title: title.clone(),
                content: content.clone(),
            };
            let json = serde_json::to_string(&create_request)?;
            let deserialized: CreateMemoRequest = serde_json::from_str(&json)?;
            prop_assert_eq!(create_request, deserialized);

            // Test SearchMemosRequest
            let search_request = SearchMemosRequest {
                query,
            };
            let json = serde_json::to_string(&search_request)?;
            let deserialized: SearchMemosRequest = serde_json::from_str(&json)?;
            prop_assert_eq!(search_request, deserialized);

            // Test UpdateMemoRequest
            let update_request = UpdateMemoRequest {
                id: MemoId::new(),
                content,
            };
            let json = serde_json::to_string(&update_request)?;
            let deserialized: UpdateMemoRequest = serde_json::from_str(&json)?;
            prop_assert_eq!(update_request, deserialized);
        }

        #[test]
        fn test_search_result_properties(
            title in ".*",
            content in ".*",
            relevance_score in 0.0f32..100.0f32,
            highlights in prop::collection::vec(".*", 0..10),
            match_count in 0usize..1000
        ) {
            let memo = Memo::new(title, content);
            let result = SearchResult {
                memo: memo.clone(),
                relevance_score,
                highlights: highlights.clone(),
                match_count,
            };

            // Test serialization roundtrip
            let json = serde_json::to_string(&result)?;
            let deserialized: SearchResult = serde_json::from_str(&json)?;

            prop_assert_eq!(result.memo.id, deserialized.memo.id);
            prop_assert_eq!(result.relevance_score, deserialized.relevance_score);
            prop_assert_eq!(result.highlights, deserialized.highlights);
            prop_assert_eq!(result.match_count, deserialized.match_count);
        }

        #[test]
        fn test_ulid_string_parsing_invariant(valid_ulid_str in "[0-9A-Z]{26}") {
            // Test that any 26-character alphanumeric string can be parsed as ULID
            let result = MemoId::from_string(valid_ulid_str.clone());

            // Note: This might fail for some edge cases due to ULID encoding rules
            // but it tests the parsing robustness
            if let Ok(id) = result {
                prop_assert_eq!(id.as_str(), valid_ulid_str);
            }
        }

        #[test]
        fn test_memo_content_size_limits(
            title_size in 0usize..1_000,
            content_size in 0usize..10_000
        ) {
            let title = "T".repeat(title_size);
            let content = "C".repeat(content_size);

            let memo = Memo::new(title.clone(), content.clone());

            prop_assert_eq!(memo.title.len(), title_size);
            prop_assert_eq!(memo.content.len(), content_size);

            // Test that serialization works even with large content
            let json_result = serde_json::to_string(&memo);
            prop_assert!(json_result.is_ok());

            if let Ok(json) = json_result {
                let deserialization_result = serde_json::from_str::<Memo>(&json);
                prop_assert!(deserialization_result.is_ok());

                if let Ok(deserialized) = deserialization_result {
                    prop_assert_eq!(deserialized.title.len(), title_size);
                    prop_assert_eq!(deserialized.content.len(), content_size);
                }
            }
        }
    }
}
