//! Rule evaluation result caching
//!
//! This module provides caching for rule evaluation results to avoid re-checking
//! unchanged file/rule pairs. Cache keys are SHA-256 hashes of file content + rule template,
//! ensuring automatic invalidation when either changes.
//!
//! ## Cache Key Strategy
//!
//! The cache uses SHA-256 hashes rather than file timestamps or modification times because:
//! - **Git operations** can reset file timestamps, causing false cache misses
//! - **File system operations** (copy, move) can preserve or modify timestamps unpredictably
//! - **Content-based hashing** provides deterministic cache keys that only change when actual content changes
//! - **Rule template inclusion** ensures cache invalidation when rule definitions are updated
//!
//! This approach trades a small hash computation cost for reliable cache behavior across
//! different workflows and environments.

use crate::{Result, RuleError, RuleViolation, Severity};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

/// Result of a cached rule evaluation
#[derive(Debug, Clone, PartialEq)]
pub enum CachedResult {
    /// Rule check passed
    Pass,
    /// Rule check found a violation
    Violation {
        /// The violation details
        violation: RuleViolation,
    },
}

/// Cache entry structure stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    /// Result of the rule evaluation
    result: CachedResultData,
    /// Timestamp when the result was cached
    timestamp: DateTime<Utc>,
}

/// Serializable version of CachedResult
#[derive(Debug, Clone, Serialize, Deserialize)]
enum CachedResultData {
    Pass,
    Violation {
        rule_name: String,
        file_path: PathBuf,
        severity: Severity,
        message: String,
    },
}

impl From<&CachedResult> for CachedResultData {
    fn from(cached_result: &CachedResult) -> Self {
        match cached_result {
            CachedResult::Pass => CachedResultData::Pass,
            CachedResult::Violation { violation } => CachedResultData::Violation {
                rule_name: violation.rule_name.clone(),
                file_path: violation.file_path.clone(),
                severity: violation.severity,
                message: violation.message.clone(),
            },
        }
    }
}

impl From<CachedResultData> for CachedResult {
    fn from(data: CachedResultData) -> Self {
        match data {
            CachedResultData::Pass => CachedResult::Pass,
            CachedResultData::Violation {
                rule_name,
                file_path,
                severity,
                message,
            } => CachedResult::Violation {
                violation: RuleViolation::new(rule_name, file_path, severity, message),
            },
        }
    }
}

/// Rule evaluation cache manager
///
/// Manages caching of rule evaluation results in `~/.cache/swissarmyhammer/rules/`.
/// Cache keys are SHA-256 hashes of file content + rule template, ensuring automatic
/// invalidation when either changes.
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer_rules::RuleCache;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let cache = RuleCache::new()?;
///
/// // Calculate cache key
/// let key = RuleCache::calculate_cache_key("file content", "rule template");
///
/// // Check if result is cached
/// if let Some(cached_result) = cache.get(&key)? {
///     println!("Cache hit!");
/// } else {
///     println!("Cache miss - will need to check with LLM");
/// }
/// # Ok(())
/// # }
/// ```
pub struct RuleCache {
    cache_dir: PathBuf,
}

impl RuleCache {
    /// Create a new RuleCache instance
    ///
    /// Creates the cache directory at `~/.cache/swissarmyhammer/rules/` if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Home directory cannot be determined
    /// - Cache directory cannot be created
    pub fn new() -> Result<Self> {
        let cache_dir = Self::get_cache_dir()?;

        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| {
                RuleError::CacheError(format!("Failed to create cache directory: {}", e))
            })?;
            tracing::debug!("Created cache directory: {}", cache_dir.display());
        }

        Ok(Self { cache_dir })
    }

    /// Get the cache directory path
    ///
    /// Returns `~/.cache/swissarmyhammer/rules/` on Unix-like systems.
    fn get_cache_dir() -> Result<PathBuf> {
        let cache_root = dirs::cache_dir().ok_or_else(|| {
            RuleError::CacheError("Failed to determine cache directory".to_string())
        })?;

        Ok(cache_root.join("swissarmyhammer").join("rules"))
    }

    /// Calculate SHA-256 cache key from file content, rule template, and severity
    ///
    /// The cache key uniquely identifies a file content + rule template + severity tuple.
    /// When any of these changes, the hash changes, automatically invalidating the cache.
    ///
    /// # Arguments
    ///
    /// * `file_content` - The content of the file being checked
    /// * `rule_template` - The template content of the rule
    /// * `severity` - The severity level of the rule
    ///
    /// # Returns
    ///
    /// A 64-character hexadecimal SHA-256 hash string
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_rules::{RuleCache, Severity};
    ///
    /// let key = RuleCache::calculate_cache_key("fn main() {}", "Check for TODO", Severity::Error);
    /// assert_eq!(key.len(), 64); // SHA-256 produces 64 hex characters
    /// ```
    pub fn calculate_cache_key(
        file_content: &str,
        rule_template: &str,
        severity: Severity,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(file_content.as_bytes());
        hasher.update(rule_template.as_bytes());
        hasher.update(format!("{:?}", severity).as_bytes());
        let hash = hasher.finalize();
        format!("{:x}", hash)
    }

    /// Get cached result for a given cache key
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key (SHA-256 hash)
    ///
    /// # Returns
    ///
    /// Returns `Some(CachedResult)` if a valid cache entry exists, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache file exists but cannot be read or parsed.
    pub fn get(&self, key: &str) -> Result<Option<CachedResult>> {
        let cache_file = self.get_cache_file_path(key);

        if !cache_file.exists() {
            tracing::trace!("Cache miss for key: {}", key);
            return Ok(None);
        }

        // Read and parse cache file
        let content = fs::read_to_string(&cache_file)
            .map_err(|e| RuleError::CacheError(format!("Failed to read cache file: {}", e)))?;

        let entry: CacheEntry = serde_json::from_str(&content)
            .map_err(|e| RuleError::CacheError(format!("Failed to parse cache file: {}", e)))?;

        tracing::debug!("Cache hit for key: {} (cached at {})", key, entry.timestamp);

        Ok(Some(entry.result.into()))
    }

    /// Store a result in the cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key (SHA-256 hash)
    /// * `result` - The result to cache
    ///
    /// # Errors
    ///
    /// Returns an error if the cache file cannot be written.
    pub fn store(&self, key: &str, result: &CachedResult) -> Result<()> {
        let cache_file = self.get_cache_file_path(key);

        let entry = CacheEntry {
            result: result.into(),
            timestamp: Utc::now(),
        };

        let content = serde_json::to_string_pretty(&entry).map_err(|e| {
            RuleError::CacheError(format!("Failed to serialize cache entry: {}", e))
        })?;

        fs::write(&cache_file, content)
            .map_err(|e| RuleError::CacheError(format!("Failed to write cache file: {}", e)))?;

        tracing::debug!("Cached result for key: {}", key);

        Ok(())
    }

    /// Clear all cache entries
    ///
    /// Removes all cache files from the cache directory.
    ///
    /// # Returns
    ///
    /// Returns the number of cache entries cleared.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache directory cannot be read or files cannot be deleted.
    pub fn clear(&self) -> Result<usize> {
        if !self.cache_dir.exists() {
            tracing::info!("Cache directory does not exist, nothing to clear");
            return Ok(0);
        }

        let mut count = 0;

        for entry in fs::read_dir(&self.cache_dir)
            .map_err(|e| RuleError::CacheError(format!("Failed to read cache directory: {}", e)))?
        {
            let entry = entry.map_err(|e| {
                RuleError::CacheError(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("cache") {
                fs::remove_file(&path).map_err(|e| {
                    RuleError::CacheError(format!("Failed to remove cache file: {}", e))
                })?;
                count += 1;
            }
        }

        tracing::info!("Cleared {} cache entries", count);

        Ok(count)
    }

    /// Get the file path for a cache entry
    fn get_cache_file_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.cache", key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_cache() -> (RuleCache, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache = RuleCache {
            cache_dir: temp_dir.path().to_path_buf(),
        };
        fs::create_dir_all(&cache.cache_dir).unwrap();
        (cache, temp_dir)
    }

    #[test]
    fn test_calculate_cache_key() {
        let key1 = RuleCache::calculate_cache_key("content1", "rule1", Severity::Error);
        let key2 = RuleCache::calculate_cache_key("content1", "rule1", Severity::Error);
        let key3 = RuleCache::calculate_cache_key("content2", "rule1", Severity::Error);
        let key4 = RuleCache::calculate_cache_key("content1", "rule2", Severity::Error);
        let key5 = RuleCache::calculate_cache_key("content1", "rule1", Severity::Warning);

        // Same inputs produce same key
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 64); // SHA-256 is 64 hex chars

        // Different inputs produce different keys
        assert_ne!(key1, key3);
        assert_ne!(key1, key4);
        assert_ne!(key1, key5); // Different severity produces different key
    }

    #[test]
    fn test_cache_store_and_get_pass() {
        let (cache, _temp_dir) = create_test_cache();

        let key = RuleCache::calculate_cache_key("test content", "test rule", Severity::Error);
        let result = CachedResult::Pass;

        // Store result
        cache.store(&key, &result).unwrap();

        // Retrieve result
        let retrieved = cache.get(&key).unwrap();
        assert_eq!(retrieved, Some(CachedResult::Pass));
    }

    #[test]
    fn test_cache_store_and_get_violation() {
        let (cache, _temp_dir) = create_test_cache();

        let key = RuleCache::calculate_cache_key("test content", "test rule", Severity::Error);
        let violation = RuleViolation::new(
            "test-rule".to_string(),
            PathBuf::from("test.rs"),
            Severity::Error,
            "Test violation message".to_string(),
        );
        let result = CachedResult::Violation { violation };

        // Store result
        cache.store(&key, &result).unwrap();

        // Retrieve result
        let retrieved = cache.get(&key).unwrap().unwrap();
        match retrieved {
            CachedResult::Violation { violation: v } => {
                assert_eq!(v.rule_name, "test-rule");
                assert_eq!(v.file_path, PathBuf::from("test.rs"));
                assert_eq!(v.severity, Severity::Error);
                assert_eq!(v.message, "Test violation message");
            }
            _ => panic!("Expected Violation result"),
        }
    }

    #[test]
    fn test_cache_miss() {
        let (cache, _temp_dir) = create_test_cache();

        let key = RuleCache::calculate_cache_key("nonexistent", "content", Severity::Error);
        let result = cache.get(&key).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_cache_clear() {
        let (cache, _temp_dir) = create_test_cache();

        // Store multiple results
        let key1 = RuleCache::calculate_cache_key("content1", "rule1", Severity::Error);
        let key2 = RuleCache::calculate_cache_key("content2", "rule2", Severity::Error);

        cache.store(&key1, &CachedResult::Pass).unwrap();
        cache.store(&key2, &CachedResult::Pass).unwrap();

        // Verify they exist
        assert!(cache.get(&key1).unwrap().is_some());
        assert!(cache.get(&key2).unwrap().is_some());

        // Clear cache
        let count = cache.clear().unwrap();
        assert_eq!(count, 2);

        // Verify they're gone
        assert!(cache.get(&key1).unwrap().is_none());
        assert!(cache.get(&key2).unwrap().is_none());
    }

    #[test]
    fn test_cache_clear_empty_directory() {
        let (cache, _temp_dir) = create_test_cache();

        // Clear empty cache
        let count = cache.clear().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_cache_invalidation_on_file_change() {
        let (cache, _temp_dir) = create_test_cache();

        let key1 = RuleCache::calculate_cache_key("original content", "rule", Severity::Error);
        let key2 = RuleCache::calculate_cache_key("modified content", "rule", Severity::Error);

        // Keys should be different when file content changes
        assert_ne!(key1, key2);

        cache.store(&key1, &CachedResult::Pass).unwrap();

        // Changed file content means different key, so cache miss
        assert!(cache.get(&key2).unwrap().is_none());
    }

    #[test]
    fn test_cache_invalidation_on_rule_change() {
        let (cache, _temp_dir) = create_test_cache();

        let key1 = RuleCache::calculate_cache_key("content", "original rule", Severity::Error);
        let key2 = RuleCache::calculate_cache_key("content", "modified rule", Severity::Error);

        // Keys should be different when rule changes
        assert_ne!(key1, key2);

        cache.store(&key1, &CachedResult::Pass).unwrap();

        // Changed rule means different key, so cache miss
        assert!(cache.get(&key2).unwrap().is_none());
    }

    #[test]
    fn test_cache_invalidation_on_severity_change() {
        let (cache, _temp_dir) = create_test_cache();

        let key1 = RuleCache::calculate_cache_key("content", "rule", Severity::Error);
        let key2 = RuleCache::calculate_cache_key("content", "rule", Severity::Warning);

        // Keys should be different when severity changes
        assert_ne!(key1, key2);

        cache.store(&key1, &CachedResult::Pass).unwrap();

        // Changed severity means different key, so cache miss
        assert!(cache.get(&key2).unwrap().is_none());
    }

    #[test]
    fn test_cache_entry_includes_timestamp() {
        let (cache, _temp_dir) = create_test_cache();

        let key = RuleCache::calculate_cache_key("content", "rule", Severity::Error);
        let before = Utc::now();

        cache.store(&key, &CachedResult::Pass).unwrap();

        let after = Utc::now();

        // Read the cache file directly to check timestamp
        let cache_file = cache.get_cache_file_path(&key);
        let content = fs::read_to_string(cache_file).unwrap();
        let entry: CacheEntry = serde_json::from_str(&content).unwrap();

        assert!(entry.timestamp >= before && entry.timestamp <= after);
    }

    #[test]
    fn test_cached_result_equality() {
        let pass1 = CachedResult::Pass;
        let pass2 = CachedResult::Pass;
        assert_eq!(pass1, pass2);

        let violation1 = CachedResult::Violation {
            violation: RuleViolation::new(
                "rule".to_string(),
                PathBuf::from("test.rs"),
                Severity::Error,
                "message".to_string(),
            ),
        };
        let violation2 = CachedResult::Violation {
            violation: RuleViolation::new(
                "rule".to_string(),
                PathBuf::from("test.rs"),
                Severity::Error,
                "message".to_string(),
            ),
        };
        assert_eq!(violation1, violation2);

        assert_ne!(pass1, violation1);
    }
}
