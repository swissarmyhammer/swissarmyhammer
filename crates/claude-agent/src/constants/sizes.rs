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

/// How much longer a permissive length bound is than the default one.
///
/// The path bound and the URI bound widen together, so one factor names both.
/// Written once here, a change to it moves the whole permissive tier as a set
/// instead of leaving one length behind.
pub const PERMISSIVE_LIMIT_MULTIPLIER: usize = 2;

/// File system limits
pub mod fs {
    use super::PERMISSIVE_LIMIT_MULTIPLIER;

    /// Maximum path length (4KB)
    pub const MAX_PATH_LENGTH: usize = 4096;

    /// Strict path length limit for sensitive operations (1KB)
    pub const MAX_PATH_LENGTH_STRICT: usize = 1024;

    /// Permissive path length limit for trusted operations (8KB)
    pub const MAX_PATH_LENGTH_PERMISSIVE: usize = MAX_PATH_LENGTH * PERMISSIVE_LIMIT_MULTIPLIER;
}

/// URI and URL limits
pub mod uri {
    use super::PERMISSIVE_LIMIT_MULTIPLIER;

    /// Standard maximum URI length (4KB)
    pub const MAX_URI_LENGTH: usize = 4096;

    /// Extended URI length, which the default limits use (8KB)
    pub const MAX_URI_LENGTH_EXTENDED: usize = 8192;

    /// Permissive URI length limit for trusted operations (16KB)
    pub const MAX_URI_LENGTH_PERMISSIVE: usize =
        MAX_URI_LENGTH_EXTENDED * PERMISSIVE_LIMIT_MULTIPLIER;
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

    /// How much smaller the strict metadata bound is than the default one.
    ///
    /// The other strict bounds each have a constant of their own. The metadata
    /// bound has none, so the strict tier divides the default by this factor
    /// and the relation stays visible.
    pub const STRICT_META_SIZE_DIVISOR: usize = 10;

    /// Strict mode metadata object size (10KB)
    pub const MAX_META_SIZE_STRICT: usize = MAX_META_SIZE / STRICT_META_SIZE_DIVISOR;

    /// How much larger the permissive metadata bound is than the default one.
    ///
    /// The metadata bound widens by a factor of its own, not by
    /// [`super::PERMISSIVE_LIMIT_MULTIPLIER`], because a `_meta` object grows
    /// with the number of keys a client sends rather than with a path length.
    pub const PERMISSIVE_META_MULTIPLIER: usize = 10;

    /// Permissive mode metadata object size (1,000,000 bytes)
    pub const MAX_META_SIZE_PERMISSIVE: usize = MAX_META_SIZE * PERMISSIVE_META_MULTIPLIER;
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
    /// The maximum prompt text one turn may carry, in **bytes** (512 KiB).
    ///
    /// This is the workspace's single declaration of the agent prompt cap. It
    /// is read by three places that MUST agree, and by no others:
    ///
    /// 1. [`AgentConfig::max_prompt_length`](crate::config::AgentConfig::max_prompt_length)'s
    ///    default — the value
    ///    `ClaudeAgent::validate_prompt_request` rejects a prompt against.
    /// 2. `swissarmyhammer_agent`'s Claude agent config, which used to carry
    ///    its own `MAX_PROMPT_LENGTH_BYTES = 5_000_000`.
    /// 3. The review engine's batch budget
    ///    (`swissarmyhammer_validators::review::AGENT_PROMPT_CAP`), which packs
    ///    a batch's RENDERED prompt to stay inside this number.
    ///
    /// They were three independent numbers (100_000 / 5_000_000 / a 384 KiB
    /// raw-source batch budget), so the effective cap depended on which agent
    /// served the run and the batcher budgeted against a number unrelated to
    /// either. A review batch packed ~15 MB against a 5 MB cap and every fat
    /// task came back as a bare `invalid_params` (see `^6jsxjbc`).
    ///
    /// # Why 512 KiB
    ///
    /// Sized to the 200k-token context window the Claude models expose. At a
    /// conservative ~3.5 bytes/token for source-heavy prompts, leaving ~25% of
    /// the window for the reply: `200_000 * 0.75 * 3.5 ≈ 525_000` bytes → 512
    /// KiB. A cap above that is not a cap at all — the prompt is accepted here
    /// and then rejected by the model for exceeding its context, which is the
    /// same silent failure one layer down.
    pub const MAX_PROMPT_LENGTH: usize = 512 * 1024;

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
