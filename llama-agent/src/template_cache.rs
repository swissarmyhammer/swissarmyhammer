//! Template caching system for llama-agent
//!
//! This module implements a persistent cache for chat template rendering and KV cache state.
//! By caching the template prefix (system prompt + tool definitions), subsequent sessions
//! can skip re-processing these tokens, significantly improving performance.
//!
//! # Architecture
//!
//! The template cache consists of two components:
//!
//! 1. **Metadata Cache**: In-memory HashMap mapping template content hashes to cache entries
//! 2. **KV Cache Files**: On-disk files containing llama.cpp KV cache state for each template
//!
//! When a chat session starts, the system prompt and tool definitions are hashed. If a cache
//! entry exists, the KV cache is loaded from disk and the model can immediately begin generating
//! from the cached position, avoiding re-processing potentially thousands of tokens.
//!
//! # Cache Invalidation
//!
//! Cache entries are invalidated when:
//! - The system prompt changes
//! - The tool definitions change
//! - Cache files are manually deleted
//!
//! The cache uses content-based hashing, so any modification to the template automatically
//! creates a new cache entry.
//!
//! # Performance Characteristics
//!
//! - **Cache Hit**: O(1) lookup + disk I/O for KV cache load
//! - **Cache Miss**: Full template rendering required (hundreds to thousands of tokens)
//! - **Memory Usage**: Minimal (only metadata stored in memory)
//! - **Disk Usage**: Proportional to number of unique templates and model KV cache size
//!
//! # Thread Safety
//!
//! `TemplateCache` is not thread-safe by default. For concurrent access, wrap in
//! `Arc<Mutex<TemplateCache>>` or use per-thread cache instances.
//!
//! # Example
//!
//! ```no_run
//! use llama_agent::TemplateCache;
//! use std::path::PathBuf;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create cache in application cache directory
//! let cache_dir = PathBuf::from("/tmp/llama_cache");
//! let mut cache = TemplateCache::new(cache_dir)?;
//!
//! // Compute hash for current template
//! let system_prompt = "You are a helpful assistant.";
//! let tools_json = r#"[{"name": "get_weather", "parameters": {}}]"#;
//! let hash = TemplateCache::hash_template(system_prompt, tools_json);
//!
//! // Check if cached
//! if let Some(entry) = cache.get(hash) {
//!     println!("Cache hit! Skip {} tokens", entry.token_count);
//!     // Load KV cache from entry.kv_cache_file
//! } else {
//!     println!("Cache miss - rendering template");
//!     // Render template, get token count
//!     let token_count = 1234; // from rendering
//!     let kv_file = cache.insert(hash, token_count, system_prompt.to_string(), tools_json.to_string());
//!     // Save KV cache to kv_file
//! }
//!
//! // Check statistics
//! let stats = cache.stats();
//! println!("Hit rate: {:.1}%", stats.hit_rate * 100.0);
//! # Ok(())
//! # }
//! ```

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::debug;

/// Entry in the template cache
///
/// Represents a cached template with its associated KV cache file and metadata.
#[derive(Debug, Clone)]
pub struct TemplateCacheEntry {
    /// Path to saved KV cache file
    pub kv_cache_file: PathBuf,

    /// Number of tokens in the template
    /// Messages should start decoding at this position
    pub token_count: usize,

    /// Raw template content for verification
    pub system_prompt: String,
    pub tools_json: String,

    /// Metadata
    pub created_at: SystemTime,
    pub last_used: SystemTime,
}

/// Global cache mapping template hashes to saved KV cache files
///
/// Manages a collection of cached template renderings with their associated KV cache files.
/// Tracks usage statistics and provides efficient lookup by template content hash.
///
/// # Cache Eviction
///
/// When the cache reaches `max_entries`, the least recently used entry is evicted.
/// Set `max_entries` to `None` for unbounded cache (not recommended for production).
///
/// # Thread Safety
///
/// This type is NOT thread-safe. For concurrent access, wrap in `Arc<Mutex<TemplateCache>>`.
pub struct TemplateCache {
    /// Map: template_hash → cache entry
    cache: HashMap<u64, TemplateCacheEntry>,

    /// Directory where KV cache files are stored
    cache_dir: PathBuf,

    /// Maximum number of cache entries (None = unbounded)
    max_entries: Option<usize>,

    /// Statistics
    hits: u64,
    misses: u64,
    evictions: u64,
}

impl TemplateCache {
    /// Default maximum number of cache entries
    pub const DEFAULT_MAX_ENTRIES: usize = 100;

    /// Create new template cache with specified directory
    ///
    /// Creates the cache directory if it doesn't exist.
    /// Uses default max entries limit of 100.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory path where KV cache files will be stored
    ///
    /// # Errors
    ///
    /// Returns error if cache directory cannot be created or accessed.
    pub fn new(cache_dir: PathBuf) -> Result<Self, TemplateCacheError> {
        Self::with_max_entries(cache_dir, Some(Self::DEFAULT_MAX_ENTRIES))
    }

    /// Create new template cache with custom maximum entries
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory path where KV cache files will be stored
    /// * `max_entries` - Maximum number of entries (None = unbounded)
    ///
    /// # Errors
    ///
    /// Returns error if cache directory cannot be created or accessed.
    pub fn with_max_entries(
        cache_dir: PathBuf,
        max_entries: Option<usize>,
    ) -> Result<Self, TemplateCacheError> {
        std::fs::create_dir_all(&cache_dir)?;

        Ok(Self {
            cache: HashMap::new(),
            cache_dir,
            max_entries,
            hits: 0,
            misses: 0,
            evictions: 0,
        })
    }

    /// Hash template content to create cache key
    ///
    /// Computes a deterministic hash from system prompt and tool definitions.
    /// Identical inputs always produce the same hash.
    ///
    /// # Arguments
    ///
    /// * `system_prompt` - System prompt text
    /// * `tools_json` - Tool definitions as JSON string
    ///
    /// # Returns
    ///
    /// 64-bit hash value uniquely identifying this template combination
    pub fn hash_template(system_prompt: &str, tools_json: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        system_prompt.hash(&mut hasher);
        tools_json.hash(&mut hasher);
        hasher.finish()
    }

    /// Check if template is cached
    ///
    /// Updates last_used timestamp on cache hit.
    ///
    /// # Arguments
    ///
    /// * `template_hash` - Hash value from `hash_template()`
    ///
    /// # Returns
    ///
    /// Cache entry if found, None otherwise
    pub fn get(&mut self, template_hash: u64) -> Option<&TemplateCacheEntry> {
        if let Some(entry) = self.cache.get_mut(&template_hash) {
            entry.last_used = SystemTime::now();
            self.hits += 1;
            debug!(
                "Template cache HIT: {} ({} tokens from {})",
                template_hash,
                entry.token_count,
                entry.kv_cache_file.display()
            );
            Some(entry)
        } else {
            self.misses += 1;
            debug!("Template cache MISS: {}", template_hash);
            None
        }
    }

    /// Store template cache metadata and return file path for KV cache
    ///
    /// Creates a new cache entry and returns the file path where the KV cache
    /// should be saved. The caller is responsible for actually saving the KV cache.
    ///
    /// # Arguments
    ///
    /// * `template_hash` - Hash value from `hash_template()`
    /// * `token_count` - Number of tokens in the rendered template (must be > 0)
    /// * `system_prompt` - System prompt text (for verification)
    /// * `tools_json` - Tool definitions JSON (for verification)
    ///
    /// # Returns
    ///
    /// Path where KV cache file should be saved
    ///
    /// # Errors
    ///
    /// Returns `TemplateCacheError::ValidationError` if:
    /// - `token_count` is 0
    /// - `system_prompt` is empty
    pub fn insert(
        &mut self,
        template_hash: u64,
        token_count: usize,
        system_prompt: String,
        tools_json: String,
    ) -> Result<PathBuf, TemplateCacheError> {
        // Validate inputs
        if token_count == 0 {
            return Err(TemplateCacheError::ValidationError(
                "token_count must be greater than 0".to_string(),
            ));
        }

        if system_prompt.is_empty() {
            return Err(TemplateCacheError::ValidationError(
                "system_prompt cannot be empty".to_string(),
            ));
        }

        let filename = format!("template_{:016x}.kv", template_hash);
        let kv_cache_file = self.cache_dir.join(filename);

        debug!(
            "Caching template {} ({} tokens) to {}",
            template_hash,
            token_count,
            kv_cache_file.display()
        );

        let entry = TemplateCacheEntry {
            kv_cache_file: kv_cache_file.clone(),
            token_count,
            system_prompt,
            tools_json,
            created_at: SystemTime::now(),
            last_used: SystemTime::now(),
        };

        // Check if we need to evict before inserting
        if let Some(max) = self.max_entries {
            if self.cache.len() >= max {
                self.evict_lru()?;
            }
        }

        self.cache.insert(template_hash, entry);
        Ok(kv_cache_file)
    }

    /// Evict the least recently used cache entry
    fn evict_lru(&mut self) -> Result<(), TemplateCacheError> {
        if self.cache.is_empty() {
            return Ok(());
        }

        // Find LRU entry
        let lru_hash = self
            .cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(hash, _)| *hash)
            .ok_or_else(|| {
                TemplateCacheError::ValidationError("Cannot evict from empty cache".to_string())
            })?;

        debug!("Evicting LRU template cache entry: {}", lru_hash);

        // Delete the entry
        self.delete(lru_hash)?;
        self.evictions += 1;

        Ok(())
    }

    /// Get cache statistics
    ///
    /// Returns current cache metrics including hit rate and token counts.
    pub fn stats(&self) -> CacheStats {
        let total_tokens: usize = self.cache.values().map(|e| e.token_count).sum();

        let hit_rate = if self.hits + self.misses > 0 {
            self.hits as f64 / (self.hits + self.misses) as f64
        } else {
            0.0
        };

        CacheStats {
            entries: self.cache.len(),
            max_entries: self.max_entries,
            total_tokens,
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            hit_rate,
        }
    }

    /// Get cache directory path
    ///
    /// Returns the directory where KV cache files are stored.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Check if a KV cache file exists for the given template hash
    ///
    /// # Arguments
    ///
    /// * `template_hash` - Hash value from `hash_template()`
    ///
    /// # Returns
    ///
    /// true if the KV cache file exists on disk, false otherwise
    pub fn has_kv_cache(&self, template_hash: u64) -> bool {
        if let Some(entry) = self.cache.get(&template_hash) {
            entry.kv_cache_file.exists()
        } else {
            false
        }
    }

    /// Delete KV cache file for a given template hash
    ///
    /// Removes both the cache entry and the KV cache file from disk.
    ///
    /// # Arguments
    ///
    /// * `template_hash` - Hash value from `hash_template()`
    ///
    /// # Returns
    ///
    /// true if the entry was deleted, false if it didn't exist
    ///
    /// # Errors
    ///
    /// Returns error if file deletion fails
    pub fn delete(&mut self, template_hash: u64) -> Result<bool, TemplateCacheError> {
        if let Some(entry) = self.cache.remove(&template_hash) {
            if entry.kv_cache_file.exists() {
                std::fs::remove_file(&entry.kv_cache_file)?;
                debug!("Deleted KV cache file: {}", entry.kv_cache_file.display());
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Verify that a cached entry's content matches the expected template
    ///
    /// This allows callers to verify that a cache hit is actually valid for the
    /// current system prompt and tools configuration.
    ///
    /// # Arguments
    ///
    /// * `template_hash` - Hash value from `hash_template()`
    /// * `system_prompt` - Expected system prompt text
    /// * `tools_json` - Expected tool definitions JSON
    ///
    /// # Returns
    ///
    /// true if cache entry exists and content matches, false otherwise
    pub fn verify(&self, template_hash: u64, system_prompt: &str, tools_json: &str) -> bool {
        if let Some(entry) = self.cache.get(&template_hash) {
            entry.system_prompt == system_prompt && entry.tools_json == tools_json
        } else {
            false
        }
    }
}

/// Statistics about template cache usage
///
/// Provides metrics for monitoring cache performance and efficiency.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of cached templates
    pub entries: usize,
    /// Maximum number of entries allowed (None = unbounded)
    pub max_entries: Option<usize>,
    /// Total tokens across all cached templates
    pub total_tokens: usize,
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of evictions
    pub evictions: u64,
    /// Cache hit rate (hits / (hits + misses))
    pub hit_rate: f64,
}

/// Errors that can occur during template cache operations
#[derive(Debug, thiserror::Error)]
pub enum TemplateCacheError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid cache path")]
    InvalidPath,

    #[error("Validation error: {0}")]
    ValidationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_hash_template_consistent() {
        let hash1 = TemplateCache::hash_template("system", "tools");
        let hash2 = TemplateCache::hash_template("system", "tools");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_template_different() {
        let hash1 = TemplateCache::hash_template("system1", "tools");
        let hash2 = TemplateCache::hash_template("system2", "tools");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_cache_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();
        assert_eq!(cache.cache.len(), 0);
        assert!(temp_dir.path().exists());
    }

    #[test]
    fn test_cache_miss() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

        let hash = TemplateCache::hash_template("sys", "tools");
        assert!(cache.get(hash).is_none());

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);
    }

    #[test]
    fn test_cache_insert_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

        let hash = TemplateCache::hash_template("sys", "tools");
        let path = cache
            .insert(hash, 100, "sys".to_string(), "tools".to_string())
            .unwrap();

        assert!(path.to_string_lossy().contains("template_"));
        assert!(path.to_string_lossy().ends_with(".kv"));

        let entry = cache.get(hash).unwrap();
        assert_eq!(entry.token_count, 100);
        assert_eq!(entry.system_prompt, "sys");

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn test_cache_stats() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

        let hash1 = TemplateCache::hash_template("sys1", "tools1");
        let hash2 = TemplateCache::hash_template("sys2", "tools2");

        cache
            .insert(hash1, 100, "sys1".to_string(), "tools1".to_string())
            .unwrap();
        cache
            .insert(hash2, 200, "sys2".to_string(), "tools2".to_string())
            .unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entries, 2);
        assert_eq!(stats.total_tokens, 300);
    }

    #[test]
    fn test_insert_validation_zero_tokens() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

        let hash = TemplateCache::hash_template("sys", "tools");
        let result = cache.insert(hash, 0, "sys".to_string(), "tools".to_string());

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("token_count must be greater than 0"));
    }

    #[test]
    fn test_insert_validation_empty_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

        let hash = TemplateCache::hash_template("", "tools");
        let result = cache.insert(hash, 100, "".to_string(), "tools".to_string());

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("system_prompt cannot be empty"));
    }

    #[test]
    fn test_insert_with_empty_tools() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

        let hash = TemplateCache::hash_template("sys", "");
        let result = cache.insert(hash, 100, "sys".to_string(), "".to_string());

        // Empty tools_json is valid - sessions may have no tools
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("template_"));
        assert!(path.to_string_lossy().ends_with(".kv"));
    }

    #[test]
    fn test_has_kv_cache() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

        let hash = TemplateCache::hash_template("sys", "tools");

        // Should not have cache before insert
        assert!(!cache.has_kv_cache(hash));

        // Insert cache entry
        let kv_file = cache
            .insert(hash, 100, "sys".to_string(), "tools".to_string())
            .unwrap();

        // Still no file on disk
        assert!(!cache.has_kv_cache(hash));

        // Create dummy file
        std::fs::write(&kv_file, b"dummy kv data").unwrap();

        // Now should have cache
        assert!(cache.has_kv_cache(hash));
    }

    #[test]
    fn test_delete_kv_cache() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

        let hash = TemplateCache::hash_template("sys", "tools");
        let kv_file = cache
            .insert(hash, 100, "sys".to_string(), "tools".to_string())
            .unwrap();

        // Create dummy file
        std::fs::write(&kv_file, b"dummy kv data").unwrap();

        // Verify file exists
        assert!(kv_file.exists());
        assert!(cache.has_kv_cache(hash));

        // Delete cache
        let deleted = cache.delete(hash).unwrap();
        assert!(deleted);

        // Verify file and entry are gone
        assert!(!kv_file.exists());
        assert!(!cache.has_kv_cache(hash));
        assert!(cache.get(hash).is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

        let hash = TemplateCache::hash_template("sys", "tools");
        let deleted = cache.delete(hash).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_verify_cache_entry() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

        let hash = TemplateCache::hash_template("sys", "tools");
        cache
            .insert(hash, 100, "sys".to_string(), "tools".to_string())
            .unwrap();

        // Verify with matching content
        assert!(cache.verify(hash, "sys", "tools"));

        // Verify with mismatched system prompt
        assert!(!cache.verify(hash, "different", "tools"));

        // Verify with mismatched tools
        assert!(!cache.verify(hash, "sys", "different"));

        // Verify nonexistent hash
        let other_hash = TemplateCache::hash_template("other", "other");
        assert!(!cache.verify(other_hash, "other", "other"));
    }

    #[test]
    fn test_cache_eviction_lru() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache =
            TemplateCache::with_max_entries(temp_dir.path().to_path_buf(), Some(3)).unwrap();

        // Insert 3 entries
        let hash1 = TemplateCache::hash_template("sys1", "tools1");
        let hash2 = TemplateCache::hash_template("sys2", "tools2");
        let hash3 = TemplateCache::hash_template("sys3", "tools3");

        let kv1 = cache
            .insert(hash1, 100, "sys1".to_string(), "tools1".to_string())
            .unwrap();
        std::fs::write(&kv1, b"dummy1").unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let kv2 = cache
            .insert(hash2, 200, "sys2".to_string(), "tools2".to_string())
            .unwrap();
        std::fs::write(&kv2, b"dummy2").unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let kv3 = cache
            .insert(hash3, 300, "sys3".to_string(), "tools3".to_string())
            .unwrap();
        std::fs::write(&kv3, b"dummy3").unwrap();

        // All 3 should be in cache
        assert_eq!(cache.cache.len(), 3);
        assert!(cache.has_kv_cache(hash1));
        assert!(cache.has_kv_cache(hash2));
        assert!(cache.has_kv_cache(hash3));

        // Access hash2 to update its last_used time
        cache.get(hash2);
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Insert 4th entry - should evict hash1 (least recently used)
        let hash4 = TemplateCache::hash_template("sys4", "tools4");
        let kv4 = cache
            .insert(hash4, 400, "sys4".to_string(), "tools4".to_string())
            .unwrap();
        std::fs::write(&kv4, b"dummy4").unwrap();

        // hash1 should be evicted
        assert_eq!(cache.cache.len(), 3);
        assert!(!cache.has_kv_cache(hash1));
        assert!(!kv1.exists());

        // Others should still be present
        assert!(cache.has_kv_cache(hash2));
        assert!(cache.has_kv_cache(hash3));
        assert!(cache.has_kv_cache(hash4));

        // Check eviction count
        let stats = cache.stats();
        assert_eq!(stats.evictions, 1);
    }

    #[test]
    fn test_cache_unbounded() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache =
            TemplateCache::with_max_entries(temp_dir.path().to_path_buf(), None).unwrap();

        // Insert many entries
        for i in 0..150 {
            let hash = TemplateCache::hash_template(&format!("sys{}", i), &format!("tools{}", i));
            cache
                .insert(hash, 100, format!("sys{}", i), format!("tools{}", i))
                .unwrap();
        }

        // All should be in cache (no eviction)
        assert_eq!(cache.cache.len(), 150);
        let stats = cache.stats();
        assert_eq!(stats.evictions, 0);
    }

    #[test]
    fn test_cache_stats_includes_max_entries() {
        let temp_dir = TempDir::new().unwrap();
        let cache =
            TemplateCache::with_max_entries(temp_dir.path().to_path_buf(), Some(50)).unwrap();

        let stats = cache.stats();
        assert_eq!(stats.max_entries, Some(50));
    }
}
