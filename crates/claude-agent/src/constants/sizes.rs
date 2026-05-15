//! Size limit constants for the agent
//!
//! # Size Limit Constants
//!
//! This module defines size limits organized by security level and purpose.
//!
//! ## Security Levels
//! - **Strict**: Minimal limits for maximum security
//! - **Moderate**: Balanced limits for typical use (default)
//! - **Permissive**: Generous limits for trusted environments
//!
//! ## Rationale
//! - 1MB strict: Prevents most DoS attacks while allowing typical content
//! - 10MB moderate: Handles images and small files comfortably
//! - 100MB permissive: Supports larger files in trusted contexts

/// File system limits
pub mod fs {
    /// Maximum path length (4KB)
    pub const MAX_PATH_LENGTH: usize = 4096;

    /// Strict path length limit for sensitive operations (1KB)
    pub const MAX_PATH_LENGTH_STRICT: usize = 1024;
}

/// URI and URL limits
pub mod uri {
    /// Standard maximum URI length (4KB)
    pub const MAX_URI_LENGTH: usize = 4096;

    /// Extended URI length for permissive mode (8KB)
    pub const MAX_URI_LENGTH_EXTENDED: usize = 8192;
}

/// Content size limits by security level
pub mod content {
    /// Base unit for content sizes (1KB)
    pub const KB: usize = 1024;

    /// Base unit for content sizes (1MB)
    pub const MB: usize = 1024 * KB;

    /// Strict mode content limit (1MB)
    pub const MAX_CONTENT_STRICT: usize = MB;

    /// Moderate mode content limit (10MB)
    pub const MAX_CONTENT_MODERATE: usize = 10 * MB;

    /// Permissive mode content limit (100MB)
    pub const MAX_CONTENT_PERMISSIVE: usize = 100 * MB;

    /// Strict mode resource limit (5MB)
    pub const MAX_RESOURCE_STRICT: usize = 5 * MB;

    /// Moderate mode resource limit (50MB)
    pub const MAX_RESOURCE_MODERATE: usize = 50 * MB;

    /// Permissive mode resource limit (500MB)
    pub const MAX_RESOURCE_PERMISSIVE: usize = 500 * MB;

    /// Maximum metadata object size (100KB)
    pub const MAX_META_SIZE: usize = 100_000;
}

/// Buffer and channel sizes
pub mod buffers {
    /// Default notification channel buffer size
    pub const NOTIFICATION_BUFFER: usize = 32;

    /// Large notification channel buffer (for high-traffic scenarios)
    pub const NOTIFICATION_BUFFER_LARGE: usize = 1000;

    /// Cancellation channel buffer size
    pub const CANCELLATION_BUFFER: usize = 100;

    /// Duplex stream buffer size
    pub const DUPLEX_STREAM_BUFFER: usize = 1024;
}

/// Message and token limits
pub mod messages {
    /// Maximum prompt length in characters (100K)
    pub const MAX_PROMPT_LENGTH: usize = 100_000;

    /// Maximum tokens per turn (100K)
    pub const MAX_TOKENS_PER_TURN: usize = 100_000;

    /// Maximum history messages to retain
    pub const MAX_HISTORY_MESSAGES: usize = 10_000;

    /// Maximum content array length
    pub const MAX_CONTENT_ARRAY_LENGTH: usize = 1000;
}

/// Memory limits
pub mod memory {
    use super::content::MB;

    /// Maximum memory usage for base64 processing (50MB)
    pub const MAX_BASE64_MEMORY: usize = 50 * MB;
}
